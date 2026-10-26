#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>

#include "runtime.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <cstdarg>
#include <cstdlib>
#include <cstdio>
#include <unordered_map>

// Log-only socket trace for the LAN netplay work, on with SWOFTY_NET_TRACE=1. The exe imports ws2_32
// through two import descriptors: the clear code's table, and a second one that the .data1 bodies
// (DirtySock bind/sendto/connect among them) call through. Both are hooked in the IAT. WSASendTo and
// WSAEventSelect are not imported, so they are hooked inline on the ws2_32 exports.
// SWOFTY_NET_TRACE=inline hooks every export inline instead, and SWOFTY_NET_TRACE_BYTES=<n> logs up to
// n payload bytes instead of 16. Every hook calls the original with the caller's arguments, returns its
// result and restores the thread's last error before returning.

namespace {

enum Call {
    C_SOCKET,
    C_WSASOCKETW,
    C_BIND,
    C_CONNECT,
    C_SETSOCKOPT,
    C_IOCTLSOCKET,
    C_GETSOCKNAME,
    C_SENDTO,
    C_SEND,
    C_WSASENDTO,
    C_RECVFROM,
    C_RECV,
    C_WSARECVFROM,
    C_WSARECV,
    C_WSAGETOVERLAPPEDRESULT,
    C_WSAWAITFORMULTIPLEEVENTS,
    C_WSAEVENTSELECT,
    C_WSACREATEEVENT,
    C_WSAIOCTL,
    C_CLOSESOCKET,
    C_COUNT
};

struct Hook {
    const char *name;
    void *detour;
    void *orig;
    void *target;
    int slots;
    safetyhook::InlineHook inl;
};

Hook hooks[C_COUNT];

template <class F> F orig(Call c)
{
    return reinterpret_cast<F>(hooks[c].orig);
}

struct SockInfo {
    int family = 0, type = 0, proto = 0;
    sockaddr_storage local{}, peer{};
    bool probed = false;
};

struct Post {
    SOCKET s;
    char *buf;
    ULONG len;
    sockaddr *from;
    int *fromlen;
    bool recvfrom;
};

SRWLOCK lock = SRWLOCK_INIT;
HANDLE file = INVALID_HANDLE_VALUE;
std::unordered_map<uint64_t, uint32_t> counts;
std::unordered_map<SOCKET, SockInfo> socks;
std::unordered_map<void *, Post> posts;
uintptr_t exe_lo, exe_hi;
LARGE_INTEGER qpf, qpc0;
thread_local int busy;
long long hex_max = 16;

struct Guard {
    DWORD err = GetLastError();
    Guard() { busy++; }
    ~Guard()
    {
        busy--;
        SetLastError(err);
    }
};

void write_line(const char *s, size_t n)
{
    DWORD w;
    if (file != INVALID_HANDLE_VALUE)
        WriteFile(file, s, DWORD(n), &w, nullptr);
}

void raw(const char *fmt, ...)
{
    char line[1024];
    va_list ap;
    va_start(ap, fmt);
    int n = vsnprintf(line, sizeof line - 1, fmt, ap);
    va_end(ap);
    if (n < 0)
        return;
    size_t len = std::min<size_t>(size_t(n), sizeof line - 2);
    line[len++] = '\n';
    AcquireSRWLockExclusive(&lock);
    write_line(line, len);
    ReleaseSRWLockExclusive(&lock);
}

uint16_t be16(uint16_t v)
{
    return uint16_t((v >> 8) | (v << 8));
}

bool readable(int r, DWORD err)
{
    return r != SOCKET_ERROR || err != WSAEFAULT;
}

std::string addr(const sockaddr *a, int len = -1)
{
    char b[96];
    if (!a)
        return "-";
    if (a->sa_family == AF_INET && (len < 0 || len >= int(sizeof(sockaddr_in)))) {
        auto *in = reinterpret_cast<const sockaddr_in *>(a);
        auto *ip = reinterpret_cast<const uint8_t *>(&in->sin_addr);
        snprintf(b, sizeof b, "%u.%u.%u.%u:%u", ip[0], ip[1], ip[2], ip[3], be16(in->sin_port));
        return b;
    }
    if (a->sa_family == AF_INET6 && (len < 0 || len >= int(sizeof(sockaddr_in6)))) {
        auto *in = reinterpret_cast<const sockaddr_in6 *>(a);
        auto *w = reinterpret_cast<const uint8_t *>(&in->sin6_addr);
        int k = snprintf(b, sizeof b, "[");
        for (int i = 0; i < 16; i += 2)
            k += snprintf(b + k, sizeof b - k, "%s%x", i ? ":" : "", (w[i] << 8) | w[i + 1]);
        snprintf(b + k, sizeof b - k, "]:%u", be16(in->sin6_port));
        return b;
    }
    if (a->sa_family == 0)
        return "unset";
    snprintf(b, sizeof b, "family%u", a->sa_family);
    return b;
}

std::string hexdump(const void *p, long long n)
{
    if (!p || n <= 0)
        return "-";
    static const char d[] = "0123456789abcdef";
    std::string s;
    auto *b = static_cast<const uint8_t *>(p);
    for (long long i = 0; i < std::min(n, hex_max); i++) {
        s += d[b[i] >> 4];
        s += d[b[i] & 15];
    }
    return s;
}

std::string caller(void *ra)
{
    char b[160];
    auto a = reinterpret_cast<uintptr_t>(ra);
    if (a >= exe_lo && a < exe_hi) {
        snprintf(b, sizeof b, "exe+0x%llx", (unsigned long long)(a - exe_lo));
        return b;
    }
    HMODULE m = nullptr;
    wchar_t path[MAX_PATH];
    if (GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                           reinterpret_cast<LPCWSTR>(ra), &m) &&
        GetModuleFileNameW(m, path, MAX_PATH)) {
        const wchar_t *base = wcsrchr(path, L'\\');
        snprintf(b, sizeof b, "%ls+0x%llx", base ? base + 1 : path,
                 (unsigned long long)(a - reinterpret_cast<uintptr_t>(m)));
        return b;
    }
    snprintf(b, sizeof b, "%p", ra);
    return b;
}

void probe(SOCKET s, SockInfo &k)
{
    if (s == INVALID_SOCKET || (k.probed && k.local.ss_family))
        return;
    bool first = !k.probed;
    k.probed = true;
    HMODULE ws = GetModuleHandleW(L"ws2_32.dll");
    if (!ws)
        return;
    using NameFn = int(WSAAPI *)(SOCKET, sockaddr *, int *);
    using OptFn = int(WSAAPI *)(SOCKET, int, int, char *, int *);
    auto local = orig<NameFn>(C_GETSOCKNAME) ? orig<NameFn>(C_GETSOCKNAME)
                                              : reinterpret_cast<NameFn>(GetProcAddress(ws, "getsockname"));
    auto peer = reinterpret_cast<NameFn>(GetProcAddress(ws, "getpeername"));
    auto opt = reinterpret_cast<OptFn>(GetProcAddress(ws, "getsockopt"));
    int n = sizeof k.local;
    if (local)
        local(s, reinterpret_cast<sockaddr *>(&k.local), &n);
    n = sizeof k.peer;
    if (peer && first)
        peer(s, reinterpret_cast<sockaddr *>(&k.peer), &n);
    if (opt && first && !k.type) {
        n = sizeof k.type;
        opt(s, SOL_SOCKET, SO_TYPE, reinterpret_cast<char *>(&k.type), &n);
        k.family = k.local.ss_family;
    }
}

std::string sock_desc(SOCKET s)
{
    char b[256];
    if (s == INVALID_SOCKET)
        return "-";
    auto &k = socks[s];
    probe(s, k);
    snprintf(b, sizeof b, "sock=0x%llx af=%d type=%d proto=%d local=%s peer=%s", (unsigned long long)s, k.family,
             k.type, k.proto, addr(reinterpret_cast<sockaddr *>(&k.local)).c_str(),
             addr(reinterpret_cast<sockaddr *>(&k.peer)).c_str());
    return b;
}

void trace(Call c, SOCKET s, void *ra, int ret, DWORD err, const char *fmt, ...)
{
    char body[1536];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(body, sizeof body, fmt, ap);
    va_end(ap);
    AcquireSRWLockExclusive(&lock);
    uint32_t n = ++counts[(uint64_t(c) << 56) ^ uint64_t(s)];
    if (n <= 200 || n % 100 == 0) {
        LARGE_INTEGER q;
        QueryPerformanceCounter(&q);
        double ms = double(q.QuadPart - qpc0.QuadPart) * 1000.0 / double(qpf.QuadPart);
        std::string sd = sock_desc(s);
        char line[2048];
        int len = snprintf(line, sizeof line, "%10.3f t%05lu %s #%u %s %s ret=%d err=%lu from=%s\n", ms,
                           GetCurrentThreadId(), hooks[c].name, n, sd.c_str(), body, ret, err, caller(ra).c_str());
        if (len > 0)
            write_line(line, std::min<size_t>(size_t(len), sizeof line - 1));
    }
    ReleaseSRWLockExclusive(&lock);
}

void note_socket(SOCKET s, int af, int type, int proto)
{
    if (s == INVALID_SOCKET)
        return;
    AcquireSRWLockExclusive(&lock);
    SockInfo k;
    k.family = af, k.type = type, k.proto = proto, k.probed = true;
    socks[s] = k;
    ReleaseSRWLockExclusive(&lock);
}

void note_addr(SOCKET s, const sockaddr *a, int len, bool local)
{
    if (!a || len <= 0)
        return;
    AcquireSRWLockExclusive(&lock);
    auto &k = socks[s];
    auto &dst = local ? k.local : k.peer;
    memcpy(&dst, a, std::min<size_t>(size_t(len), sizeof dst));
    ReleaseSRWLockExclusive(&lock);
}

void note_post(void *ov, const Post &p)
{
    if (!ov)
        return;
    AcquireSRWLockExclusive(&lock);
    posts[ov] = p;
    ReleaseSRWLockExclusive(&lock);
}

bool take_post(void *ov, Post &p)
{
    AcquireSRWLockExclusive(&lock);
    auto it = posts.find(ov);
    bool ok = it != posts.end();
    if (ok)
        p = it->second;
    ReleaseSRWLockExclusive(&lock);
    return ok;
}

void *event_of(const WSAOVERLAPPED *ov)
{
    return ov ? ov->hEvent : nullptr;
}

#define RA __builtin_return_address(0)
#define PTR(x) static_cast<unsigned long long>(reinterpret_cast<uintptr_t>(x))

SOCKET WSAAPI h_socket(int af, int type, int proto)
{
    using F = SOCKET(WSAAPI *)(int, int, int);
    if (busy)
        return orig<F>(C_SOCKET)(af, type, proto);
    SOCKET r = orig<F>(C_SOCKET)(af, type, proto);
    Guard g;
    note_socket(r, af, type, proto);
    trace(C_SOCKET, r, RA, int(r == INVALID_SOCKET ? -1 : 0), r == INVALID_SOCKET ? g.err : 0, "af=%d type=%d proto=%d",
          af, type, proto);
    return r;
}

SOCKET WSAAPI h_WSASocketW(int af, int type, int proto, LPWSAPROTOCOL_INFOW info, GROUP group, DWORD flags)
{
    using F = SOCKET(WSAAPI *)(int, int, int, LPWSAPROTOCOL_INFOW, GROUP, DWORD);
    if (busy)
        return orig<F>(C_WSASOCKETW)(af, type, proto, info, group, flags);
    SOCKET r = orig<F>(C_WSASOCKETW)(af, type, proto, info, group, flags);
    Guard g;
    note_socket(r, af, type, proto);
    trace(C_WSASOCKETW, r, RA, int(r == INVALID_SOCKET ? -1 : 0), r == INVALID_SOCKET ? g.err : 0,
          "af=%d type=%d proto=%d flags=0x%lx", af, type, proto, flags);
    return r;
}

int WSAAPI h_bind(SOCKET s, const sockaddr *a, int len)
{
    using F = int(WSAAPI *)(SOCKET, const sockaddr *, int);
    if (busy)
        return orig<F>(C_BIND)(s, a, len);
    int r = orig<F>(C_BIND)(s, a, len);
    Guard g;
    if (!r)
        note_addr(s, a, len, true);
    trace(C_BIND, s, RA, r, r ? g.err : 0, "addr=%s len=%d", readable(r, g.err) ? addr(a, len).c_str() : "?", len);
    return r;
}

int WSAAPI h_connect(SOCKET s, const sockaddr *a, int len)
{
    using F = int(WSAAPI *)(SOCKET, const sockaddr *, int);
    if (busy)
        return orig<F>(C_CONNECT)(s, a, len);
    int r = orig<F>(C_CONNECT)(s, a, len);
    Guard g;
    if (readable(r, g.err))
        note_addr(s, a, len, false);
    trace(C_CONNECT, s, RA, r, r ? g.err : 0, "addr=%s len=%d", readable(r, g.err) ? addr(a, len).c_str() : "?", len);
    return r;
}

int WSAAPI h_setsockopt(SOCKET s, int level, int name, const char *val, int len)
{
    using F = int(WSAAPI *)(SOCKET, int, int, const char *, int);
    if (busy)
        return orig<F>(C_SETSOCKOPT)(s, level, name, val, len);
    int r = orig<F>(C_SETSOCKOPT)(s, level, name, val, len);
    Guard g;
    trace(C_SETSOCKOPT, s, RA, r, r ? g.err : 0, "level=0x%x name=0x%x len=%d val=%s", level, name, len,
          readable(r, g.err) ? hexdump(val, len).c_str() : "?");
    return r;
}

int WSAAPI h_ioctlsocket(SOCKET s, long cmd, u_long *arg)
{
    using F = int(WSAAPI *)(SOCKET, long, u_long *);
    if (busy)
        return orig<F>(C_IOCTLSOCKET)(s, cmd, arg);
    int r = orig<F>(C_IOCTLSOCKET)(s, cmd, arg);
    Guard g;
    trace(C_IOCTLSOCKET, s, RA, r, r ? g.err : 0, "cmd=0x%lx arg=%lu", (unsigned long)cmd,
          arg && readable(r, g.err) ? *arg : 0);
    return r;
}

int WSAAPI h_getsockname(SOCKET s, sockaddr *a, int *len)
{
    using F = int(WSAAPI *)(SOCKET, sockaddr *, int *);
    if (busy)
        return orig<F>(C_GETSOCKNAME)(s, a, len);
    int r = orig<F>(C_GETSOCKNAME)(s, a, len);
    Guard g;
    if (!r && len)
        note_addr(s, a, *len, true);
    trace(C_GETSOCKNAME, s, RA, r, r ? g.err : 0, "addr=%s", r ? "-" : addr(a, len ? *len : -1).c_str());
    return r;
}

int WSAAPI h_sendto(SOCKET s, const char *buf, int len, int flags, const sockaddr *to, int tolen)
{
    using F = int(WSAAPI *)(SOCKET, const char *, int, int, const sockaddr *, int);
    if (busy)
        return orig<F>(C_SENDTO)(s, buf, len, flags, to, tolen);
    int r = orig<F>(C_SENDTO)(s, buf, len, flags, to, tolen);
    Guard g;
    bool ok = readable(r, g.err);
    trace(C_SENDTO, s, RA, r, r < 0 ? g.err : 0, "to=%s len=%d flags=0x%x data=%s", ok ? addr(to, tolen).c_str() : "?",
          len, flags, ok ? hexdump(buf, len).c_str() : "?");
    return r;
}

int WSAAPI h_send(SOCKET s, const char *buf, int len, int flags)
{
    using F = int(WSAAPI *)(SOCKET, const char *, int, int);
    if (busy)
        return orig<F>(C_SEND)(s, buf, len, flags);
    int r = orig<F>(C_SEND)(s, buf, len, flags);
    Guard g;
    trace(C_SEND, s, RA, r, r < 0 ? g.err : 0, "len=%d flags=0x%x data=%s", len, flags,
          readable(r, g.err) ? hexdump(buf, len).c_str() : "?");
    return r;
}

int WSAAPI h_WSASendTo(SOCKET s, LPWSABUF bufs, DWORD n, LPDWORD sent, DWORD flags, const sockaddr *to, int tolen,
                       LPWSAOVERLAPPED ov, LPWSAOVERLAPPED_COMPLETION_ROUTINE cr)
{
    using F = int(WSAAPI *)(SOCKET, LPWSABUF, DWORD, LPDWORD, DWORD, const sockaddr *, int, LPWSAOVERLAPPED,
                            LPWSAOVERLAPPED_COMPLETION_ROUTINE);
    if (busy)
        return orig<F>(C_WSASENDTO)(s, bufs, n, sent, flags, to, tolen, ov, cr);
    int r = orig<F>(C_WSASENDTO)(s, bufs, n, sent, flags, to, tolen, ov, cr);
    Guard g;
    bool ok = readable(r, g.err) && bufs && n;
    trace(C_WSASENDTO, s, RA, r, r ? g.err : 0, "to=%s nbufs=%lu len0=%lu sent=%ld data=%s ov=0x%llx ev=0x%llx",
          ok ? addr(to, tolen).c_str() : "?", n, ok ? bufs[0].len : 0, !r && sent ? long(*sent) : -1L,
          ok ? hexdump(bufs[0].buf, bufs[0].len).c_str() : "?", PTR(ov), PTR(event_of(ov)));
    return r;
}

int WSAAPI h_recvfrom(SOCKET s, char *buf, int len, int flags, sockaddr *from, int *fromlen)
{
    using F = int(WSAAPI *)(SOCKET, char *, int, int, sockaddr *, int *);
    if (busy)
        return orig<F>(C_RECVFROM)(s, buf, len, flags, from, fromlen);
    int r = orig<F>(C_RECVFROM)(s, buf, len, flags, from, fromlen);
    Guard g;
    if (r >= 0)
        trace(C_RECVFROM, s, RA, r, 0, "cap=%d flags=0x%x from=%s data=%s", len, flags,
              addr(from, fromlen ? *fromlen : -1).c_str(), hexdump(buf, r).c_str());
    else
        trace(C_RECVFROM, s, RA, r, g.err, "cap=%d flags=0x%x", len, flags);
    return r;
}

int WSAAPI h_recv(SOCKET s, char *buf, int len, int flags)
{
    using F = int(WSAAPI *)(SOCKET, char *, int, int);
    if (busy)
        return orig<F>(C_RECV)(s, buf, len, flags);
    int r = orig<F>(C_RECV)(s, buf, len, flags);
    Guard g;
    trace(C_RECV, s, RA, r, r < 0 ? g.err : 0, "cap=%d flags=0x%x data=%s", len, flags,
          r > 0 ? hexdump(buf, r).c_str() : "-");
    return r;
}

int WSAAPI h_WSARecvFrom(SOCKET s, LPWSABUF bufs, DWORD n, LPDWORD got, LPDWORD flags, sockaddr *from, LPINT fromlen,
                         LPWSAOVERLAPPED ov, LPWSAOVERLAPPED_COMPLETION_ROUTINE cr)
{
    using F = int(WSAAPI *)(SOCKET, LPWSABUF, DWORD, LPDWORD, LPDWORD, sockaddr *, LPINT, LPWSAOVERLAPPED,
                            LPWSAOVERLAPPED_COMPLETION_ROUTINE);
    if (busy)
        return orig<F>(C_WSARECVFROM)(s, bufs, n, got, flags, from, fromlen, ov, cr);
    int r = orig<F>(C_WSARECVFROM)(s, bufs, n, got, flags, from, fromlen, ov, cr);
    Guard g;
    bool ok = readable(r, g.err) && bufs && n;
    Post p{s, ok ? bufs[0].buf : nullptr, ok ? bufs[0].len : 0, from, fromlen, true};
    int fl_in = ok && fromlen ? *fromlen : -1;
    if (ok)
        note_post(ov, p);
    trace(C_WSARECVFROM, s, RA, r, r ? g.err : 0,
          "nbufs=%lu buf=0x%llx cap=%lu from_buf=0x%llx fromlen=%d flags_ptr=0x%llx ov=0x%llx ev=0x%llx cr=0x%llx got=%ld", n,
          PTR(p.buf), p.len, PTR(from), fl_in, PTR(flags), PTR(ov), PTR(event_of(ov)), PTR(cr),
          !r && got ? long(*got) : -1L);
    return r;
}

int WSAAPI h_WSARecv(SOCKET s, LPWSABUF bufs, DWORD n, LPDWORD got, LPDWORD flags, LPWSAOVERLAPPED ov,
                     LPWSAOVERLAPPED_COMPLETION_ROUTINE cr)
{
    using F = int(WSAAPI *)(SOCKET, LPWSABUF, DWORD, LPDWORD, LPDWORD, LPWSAOVERLAPPED,
                            LPWSAOVERLAPPED_COMPLETION_ROUTINE);
    if (busy)
        return orig<F>(C_WSARECV)(s, bufs, n, got, flags, ov, cr);
    int r = orig<F>(C_WSARECV)(s, bufs, n, got, flags, ov, cr);
    Guard g;
    bool ok = readable(r, g.err) && bufs && n;
    Post p{s, ok ? bufs[0].buf : nullptr, ok ? bufs[0].len : 0, nullptr, nullptr, false};
    if (ok)
        note_post(ov, p);
    trace(C_WSARECV, s, RA, r, r ? g.err : 0, "nbufs=%lu buf=0x%llx cap=%lu ov=0x%llx ev=0x%llx got=%ld", n, PTR(p.buf), p.len,
          PTR(ov), PTR(event_of(ov)), !r && got ? long(*got) : -1L);
    return r;
}

BOOL WSAAPI h_WSAGetOverlappedResult(SOCKET s, LPWSAOVERLAPPED ov, LPDWORD bytes, BOOL wait, LPDWORD flags)
{
    using F = BOOL(WSAAPI *)(SOCKET, LPWSAOVERLAPPED, LPDWORD, BOOL, LPDWORD);
    if (busy)
        return orig<F>(C_WSAGETOVERLAPPEDRESULT)(s, ov, bytes, wait, flags);
    BOOL r = orig<F>(C_WSAGETOVERLAPPEDRESULT)(s, ov, bytes, wait, flags);
    Guard g;
    Post p{};
    bool known = take_post(ov, p);
    if (r && known && p.recvfrom)
        trace(C_WSAGETOVERLAPPEDRESULT, s, RA, r, 0, "ov=0x%llx ev=0x%llx bytes=%lu flags=0x%lx buf=0x%llx data=%s from_buf=0x%llx from=%s",
              PTR(ov), PTR(event_of(ov)), bytes ? *bytes : 0, flags ? *flags : 0, PTR(p.buf),
              hexdump(p.buf, bytes ? *bytes : 0).c_str(), PTR(p.from),
              addr(p.from, p.fromlen ? *p.fromlen : -1).c_str());
    else if (r && known)
        trace(C_WSAGETOVERLAPPEDRESULT, s, RA, r, 0, "ov=0x%llx ev=0x%llx bytes=%lu flags=0x%lx buf=0x%llx data=%s", PTR(ov),
              PTR(event_of(ov)), bytes ? *bytes : 0, flags ? *flags : 0, PTR(p.buf),
              hexdump(p.buf, bytes ? *bytes : 0).c_str());
    else
        trace(C_WSAGETOVERLAPPEDRESULT, s, RA, r, r ? 0 : g.err, "ov=0x%llx ev=0x%llx wait=%d bytes=%lu post=%s", PTR(ov),
              PTR(event_of(ov)), wait, r && bytes ? *bytes : 0, known ? "seen" : "unseen");
    return r;
}

DWORD WSAAPI h_WSAWaitForMultipleEvents(DWORD n, const WSAEVENT *ev, BOOL all, DWORD timeout, BOOL alertable)
{
    using F = DWORD(WSAAPI *)(DWORD, const WSAEVENT *, BOOL, DWORD, BOOL);
    if (busy)
        return orig<F>(C_WSAWAITFORMULTIPLEEVENTS)(n, ev, all, timeout, alertable);
    DWORD r = orig<F>(C_WSAWAITFORMULTIPLEEVENTS)(n, ev, all, timeout, alertable);
    Guard g;
    char list[160] = "";
    for (DWORD i = 0, k = 0; ev && i < n && i < 6; i++)
        k += snprintf(list + k, sizeof list - k, "%s0x%llx", i ? "," : "", PTR(ev[i]));
    trace(C_WSAWAITFORMULTIPLEEVENTS, INVALID_SOCKET, RA, int(r), r == WSA_WAIT_FAILED ? g.err : 0,
          "n=%lu events=%s%s all=%d timeout=%ld signalled=0x%llx", n, list, n > 6 ? ",..." : "", all, long(timeout),
          PTR(ev && r < n ? ev[r] : nullptr));
    return r;
}

int WSAAPI h_WSAEventSelect(SOCKET s, WSAEVENT e, long mask)
{
    using F = int(WSAAPI *)(SOCKET, WSAEVENT, long);
    if (busy)
        return orig<F>(C_WSAEVENTSELECT)(s, e, mask);
    int r = orig<F>(C_WSAEVENTSELECT)(s, e, mask);
    Guard g;
    trace(C_WSAEVENTSELECT, s, RA, r, r ? g.err : 0, "event=0x%llx mask=0x%lx", PTR(e), (unsigned long)mask);
    return r;
}

WSAEVENT WSAAPI h_WSACreateEvent()
{
    using F = WSAEVENT(WSAAPI *)();
    if (busy)
        return orig<F>(C_WSACREATEEVENT)();
    WSAEVENT r = orig<F>(C_WSACREATEEVENT)();
    Guard g;
    trace(C_WSACREATEEVENT, INVALID_SOCKET, RA, r ? 0 : -1, r ? 0 : g.err, "event=0x%llx", PTR(r));
    return r;
}

int WSAAPI h_WSAIoctl(SOCKET s, DWORD code, LPVOID in, DWORD inlen, LPVOID out, DWORD outlen, LPDWORD got,
                      LPWSAOVERLAPPED ov, LPWSAOVERLAPPED_COMPLETION_ROUTINE cr)
{
    using F = int(WSAAPI *)(SOCKET, DWORD, LPVOID, DWORD, LPVOID, DWORD, LPDWORD, LPWSAOVERLAPPED,
                            LPWSAOVERLAPPED_COMPLETION_ROUTINE);
    if (busy)
        return orig<F>(C_WSAIOCTL)(s, code, in, inlen, out, outlen, got, ov, cr);
    int r = orig<F>(C_WSAIOCTL)(s, code, in, inlen, out, outlen, got, ov, cr);
    Guard g;
    trace(C_WSAIOCTL, s, RA, r, r ? g.err : 0, "code=0x%lx in=%s inlen=%lu outlen=%lu got=%ld ov=0x%llx", code,
          readable(r, g.err) ? hexdump(in, inlen).c_str() : "?", inlen, outlen, !r && got ? long(*got) : -1L,
          PTR(ov));
    return r;
}

int WSAAPI h_closesocket(SOCKET s)
{
    using F = int(WSAAPI *)(SOCKET);
    if (busy)
        return orig<F>(C_CLOSESOCKET)(s);
    std::string before;
    {
        Guard g;
        AcquireSRWLockExclusive(&lock);
        before = sock_desc(s);
        ReleaseSRWLockExclusive(&lock);
    }
    int r = orig<F>(C_CLOSESOCKET)(s);
    Guard g;
    trace(C_CLOSESOCKET, INVALID_SOCKET, RA, r, r ? g.err : 0, "%s", before.c_str());
    AcquireSRWLockExclusive(&lock);
    if (!r)
        socks.erase(s);
    ReleaseSRWLockExclusive(&lock);
    return r;
}

void table()
{
    auto set = [](Call c, const char *name, void *detour) {
        hooks[c].name = name;
        hooks[c].detour = detour;
    };
    set(C_SOCKET, "socket", reinterpret_cast<void *>(&h_socket));
    set(C_WSASOCKETW, "WSASocketW", reinterpret_cast<void *>(&h_WSASocketW));
    set(C_BIND, "bind", reinterpret_cast<void *>(&h_bind));
    set(C_CONNECT, "connect", reinterpret_cast<void *>(&h_connect));
    set(C_SETSOCKOPT, "setsockopt", reinterpret_cast<void *>(&h_setsockopt));
    set(C_IOCTLSOCKET, "ioctlsocket", reinterpret_cast<void *>(&h_ioctlsocket));
    set(C_GETSOCKNAME, "getsockname", reinterpret_cast<void *>(&h_getsockname));
    set(C_SENDTO, "sendto", reinterpret_cast<void *>(&h_sendto));
    set(C_SEND, "send", reinterpret_cast<void *>(&h_send));
    set(C_WSASENDTO, "WSASendTo", reinterpret_cast<void *>(&h_WSASendTo));
    set(C_RECVFROM, "recvfrom", reinterpret_cast<void *>(&h_recvfrom));
    set(C_RECV, "recv", reinterpret_cast<void *>(&h_recv));
    set(C_WSARECVFROM, "WSARecvFrom", reinterpret_cast<void *>(&h_WSARecvFrom));
    set(C_WSARECV, "WSARecv", reinterpret_cast<void *>(&h_WSARecv));
    set(C_WSAGETOVERLAPPEDRESULT, "WSAGetOverlappedResult", reinterpret_cast<void *>(&h_WSAGetOverlappedResult));
    set(C_WSAWAITFORMULTIPLEEVENTS, "WSAWaitForMultipleEvents",
        reinterpret_cast<void *>(&h_WSAWaitForMultipleEvents));
    set(C_WSAEVENTSELECT, "WSAEventSelect", reinterpret_cast<void *>(&h_WSAEventSelect));
    set(C_WSACREATEEVENT, "WSACreateEvent", reinterpret_cast<void *>(&h_WSACreateEvent));
    set(C_WSAIOCTL, "WSAIoctl", reinterpret_cast<void *>(&h_WSAIoctl));
    set(C_CLOSESOCKET, "closesocket", reinterpret_cast<void *>(&h_closesocket));
}

int hook_iat(HMODULE ws, uint8_t *base)
{
    auto nt = reinterpret_cast<IMAGE_NT_HEADERS *>(base + reinterpret_cast<IMAGE_DOS_HEADER *>(base)->e_lfanew);
    auto &dir = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    if (!dir.VirtualAddress)
        return 0;
    int total = 0, table_no = 0;
    for (auto *d = reinterpret_cast<IMAGE_IMPORT_DESCRIPTOR *>(base + dir.VirtualAddress); d->Name; d++) {
        if (_stricmp(reinterpret_cast<const char *>(base + d->Name), "ws2_32.dll"))
            continue;
        table_no++;
        auto *iat = reinterpret_cast<IMAGE_THUNK_DATA *>(base + d->FirstThunk);
        auto *names = d->OriginalFirstThunk ? reinterpret_cast<IMAGE_THUNK_DATA *>(base + d->OriginalFirstThunk) : iat;
        int here = 0;
        for (; names->u1.AddressOfData; names++, iat++) {
            void *want = nullptr;
            char label[48];
            if (IMAGE_SNAP_BY_ORDINAL(names->u1.Ordinal)) {
                want = reinterpret_cast<void *>(
                    GetProcAddress(ws, MAKEINTRESOURCEA(IMAGE_ORDINAL(names->u1.Ordinal))));
                snprintf(label, sizeof label, "ordinal %u", unsigned(IMAGE_ORDINAL(names->u1.Ordinal)));
            } else if (d->OriginalFirstThunk) {
                auto *by = reinterpret_cast<IMAGE_IMPORT_BY_NAME *>(base + names->u1.AddressOfData);
                want = reinterpret_cast<void *>(GetProcAddress(ws, reinterpret_cast<const char *>(by->Name)));
                snprintf(label, sizeof label, "name");
            } else {
                want = reinterpret_cast<void *>(iat->u1.Function);
                snprintf(label, sizeof label, "bound");
            }
            for (auto &h : hooks) {
                if (!want || want != h.target)
                    continue;
                void *cur = reinterpret_cast<void *>(iat->u1.Function);
                if (h.orig && h.orig != cur) {
                    raw("# iat table %d slot exe+0x%llx (%s) %s holds %p, not %p; left alone", table_no,
                        (unsigned long long)(reinterpret_cast<uint8_t *>(&iat->u1.Function) - base), label, h.name, cur,
                        h.orig);
                    break;
                }
                DWORD old;
                if (!VirtualProtect(&iat->u1.Function, sizeof(void *), PAGE_READWRITE, &old))
                    break;
                h.orig = cur;
                iat->u1.Function = reinterpret_cast<ULONG_PTR>(h.detour);
                VirtualProtect(&iat->u1.Function, sizeof(void *), old, &old);
                h.slots++;
                here++;
                raw("# iat table %d slot exe+0x%llx (%s) -> %s", table_no,
                    (unsigned long long)(reinterpret_cast<uint8_t *>(&iat->u1.Function) - base), label, h.name);
                break;
            }
        }
        total += here;
    }
    return total;
}

bool hook_inline(Hook &h)
{
    if (!h.target)
        return false;
    auto made = safetyhook::InlineHook::create(h.target, h.detour);
    if (!made) {
        raw("# inline hook on ws2_32!%s at %p failed", h.name, h.target);
        return false;
    }
    h.inl = std::move(*made);
    h.orig = h.inl.trampoline().data();
    raw("# inline hook on ws2_32!%s at %p", h.name, h.target);
    return true;
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_NET_TRACE", v, sizeof v);
    if (!n || n >= sizeof v || !strcmp(v, "0"))
        return;
    bool all_inline = !_stricmp(v, "inline");
    char bytes[16] = {};
    n = GetEnvironmentVariableA("SWOFTY_NET_TRACE_BYTES", bytes, sizeof bytes);
    if (n && n < sizeof bytes)
        hex_max = std::clamp(atoll(bytes), 1ll, 512ll);
    HMODULE ws = LoadLibraryW(L"ws2_32.dll");
    if (!ws) {
        swofty::log("[net-trace] ws2_32.dll did not load (%lu); socket trace off", GetLastError());
        return;
    }
    std::wstring path = runtime::swofty_root() + L"net-trace.log";
    file = CreateFileW(path.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL,
                       nullptr);
    if (file == INVALID_HANDLE_VALUE) {
        swofty::log("[net-trace] cannot create swofty\\net-trace.log (%lu); socket trace off", GetLastError());
        return;
    }
    QueryPerformanceFrequency(&qpf);
    QueryPerformanceCounter(&qpc0);
    auto base = reinterpret_cast<uint8_t *>(GetModuleHandleW(nullptr));
    auto nt = reinterpret_cast<IMAGE_NT_HEADERS *>(base + reinterpret_cast<IMAGE_DOS_HEADER *>(base)->e_lfanew);
    exe_lo = reinterpret_cast<uintptr_t>(base);
    exe_hi = exe_lo + nt->OptionalHeader.SizeOfImage;
    table();
    for (auto &h : hooks)
        h.target = reinterpret_cast<void *>(GetProcAddress(ws, h.name));
    raw("# swofty net trace: exe timestamp 0x%08lx size 0x%lx, mode %s, first %lld data bytes; per (call, socket) the "
        "first 200 calls are logged, then every 100th",
        nt->FileHeader.TimeDateStamp, nt->OptionalHeader.SizeOfImage, all_inline ? "inline" : "iat", hex_max);
    raw("# columns: ms thread call #count sock=... <call fields> ret err from=<caller>");
    int slots = all_inline ? 0 : hook_iat(ws, base);
    std::string inl;
    for (auto &h : hooks)
        if (!h.slots && hook_inline(h))
            inl += std::string(inl.empty() ? "" : ", ") + h.name;
    std::string missing;
    for (auto &h : hooks)
        if (!h.orig)
            missing += std::string(missing.empty() ? "" : ", ") + h.name;
    raw("# ready: %d iat slot(s); inline: %s; not hooked: %s", slots, inl.empty() ? "none" : inl.c_str(),
        missing.empty() ? "none" : missing.c_str());
    swofty::log("[net-trace] socket trace on (%s): %d game import slot(s), inline %s; writing swofty\\net-trace.log",
                all_inline ? "inline" : "iat", slots, inl.empty() ? "none" : inl.c_str());
}

}

namespace runtime {

void net_trace_setup()
{
    setup();
}

}
