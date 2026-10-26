// Fake game for the socket trace test. The window trigger loads the loader and the core; with
// SWOFTY_NET_TRACE set this file then runs the receive pump's pattern (an overlapped WSARecvFrom
// waited on with WSAWaitForMultipleEvents and completed with WSAGetOverlappedResult) against a second
// socket's sendto, plus the other traced calls, and checks that every result, buffer and last error is
// what plain ws2_32 gives. WSASendTo and WSAEventSelect go through GetProcAddress so they reach the
// trace's inline hooks.
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <stdio.h>
#include <string.h>

static int fails;
static void expect(int ok, const char *what)
{
    printf("%s %s\n", ok ? "ok  :" : "FAIL:", what);
    fails += !ok;
}

static int read_log(const wchar_t *path, char *out, DWORD cap)
{
    HANDLE f = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_EXISTING, 0, NULL);
    if (f == INVALID_HANDLE_VALUE)
        return -1;
    DWORD n = 0;
    ReadFile(f, out, cap - 1, &n, NULL);
    out[n] = 0;
    CloseHandle(f);
    return (int)n;
}

static unsigned short port_of(SOCKET s)
{
    struct sockaddr_in a;
    int n = sizeof a;
    getsockname(s, (struct sockaddr *)&a, &n);
    return ntohs(a.sin_port);
}

static struct sockaddr_in loop_addr(unsigned short port)
{
    struct sockaddr_in a = {0};
    a.sin_family = AF_INET;
    a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    a.sin_port = htons(port);
    return a;
}

int main(void)
{
    static char text[1 << 20];
    wchar_t log_path[MAX_PATH];
    GetModuleFileNameW(NULL, log_path, MAX_PATH);
    wcscpy(wcsrchr(log_path, L'\\') + 1, L"swofty\\net-trace.log");
    char mode[16] = "";
    GetEnvironmentVariableA("SWOFTY_NET_TRACE", mode, sizeof mode);
    int on = mode[0] && strcmp(mode, "0");

    DWORD dummy;
    expect(GetFileVersionInfoSizeW(L"C:\\windows\\system32\\kernel32.dll", &dummy) != 0, "version.dll forwarding works");
    WNDCLASSEXW wc = {.cbSize = sizeof wc, .lpfnWndProc = DefWindowProcW, .hInstance = GetModuleHandleW(NULL),
                      .lpszClassName = L"EGO_WindowClass"};
    extern void *__imp_RegisterClassExW;
    ATOM(WINAPI * rc)(const WNDCLASSEXW *) = (void *)__imp_RegisterClassExW;
    rc(&wc);
    expect(GetModuleHandleW(L"swofty_core.dll") != NULL, "the window trigger loaded the core");

    int ready = 0;
    for (int i = 0; i < 150 && !ready; i++) {
        ready = read_log(log_path, text, sizeof text) > 0 && strstr(text, "# ready:");
        if (!ready)
            Sleep(on ? 100 : 20);
    }
    if (!on) {
        expect(!ready && read_log(log_path, text, sizeof text) < 0, "without SWOFTY_NET_TRACE no trace log is written");
        HMODULE ws = GetModuleHandleW(L"ws2_32.dll");
        extern void *__imp_sendto;
        expect(ws && __imp_sendto == (void *)GetProcAddress(ws, "sendto"), "without SWOFTY_NET_TRACE the IAT is untouched");
        printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
        ExitProcess(fails != 0);
    }
    expect(ready, "the trace wrote its ready line");

    WSADATA wd;
    expect(WSAStartup(MAKEWORD(2, 2), &wd) == 0, "WSAStartup");

    SOCKET rs = WSASocketW(AF_INET, SOCK_DGRAM, IPPROTO_UDP, NULL, 0, WSA_FLAG_OVERLAPPED);
    expect(rs != INVALID_SOCKET, "WSASocketW made the receive socket");
    int rcvbuf = 1 << 16;
    expect(setsockopt(rs, SOL_SOCKET, SO_RCVBUF, (const char *)&rcvbuf, sizeof rcvbuf) == 0, "setsockopt SO_RCVBUF");
    u_long nb = 1;
    expect(ioctlsocket(rs, FIONBIO, &nb) == 0, "ioctlsocket FIONBIO");
    struct sockaddr_in any = loop_addr(0);
    expect(bind(rs, (struct sockaddr *)&any, sizeof any) == 0, "bind the receive socket to 127.0.0.1:0");
    unsigned short rport = port_of(rs);
    expect(rport != 0, "getsockname gives the bound port");

    SOCKET ss = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    expect(ss != INVALID_SOCKET, "socket made the send socket");
    expect(bind(ss, (struct sockaddr *)&any, sizeof any) == 0, "bind the send socket");
    unsigned short sport = port_of(ss);

    struct sockaddr_in taken = loop_addr(rport);
    SOCKET dup = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    int br = bind(dup, (struct sockaddr *)&taken, sizeof taken);
    int be = WSAGetLastError();
    expect(br == SOCKET_ERROR && be == WSAEADDRINUSE, "a bind on a taken port fails with WSAEADDRINUSE after the hook");

    WSAEVENT ev = WSACreateEvent();
    expect(ev != WSA_INVALID_EVENT, "WSACreateEvent");
    WSAOVERLAPPED ov;
    memset(&ov, 0, sizeof ov);
    ov.hEvent = ev;
    char buf[0x4f0];
    WSABUF wb = {sizeof buf, buf};
    DWORD flags = 0;
    struct sockaddr_in from;
    memset(&from, 0, sizeof from);
    int fromlen = sizeof from;
    int pr = WSARecvFrom(rs, &wb, 1, NULL, &flags, (struct sockaddr *)&from, &fromlen, &ov, NULL);
    int pe = WSAGetLastError();
    expect(pr == SOCKET_ERROR && pe == WSA_IO_PENDING, "the overlapped WSARecvFrom is pending with WSA_IO_PENDING after the hook");

    static const char msg[] = "swofty-net-trace";
    struct sockaddr_in to = loop_addr(rport);
    expect(sendto(ss, msg, 16, 0, (struct sockaddr *)&to, sizeof to) == 16, "sendto the receive socket");

    DWORD w = WSAWaitForMultipleEvents(1, &ev, FALSE, 5000, FALSE);
    expect(w == WSA_WAIT_EVENT_0, "WSAWaitForMultipleEvents saw the completion");
    WSAResetEvent(ev);
    DWORD got = 0, gflags = 0;
    BOOL done = WSAGetOverlappedResult(rs, &ov, &got, FALSE, &gflags);
    expect(done && got == 16 && !memcmp(buf, msg, 16), "WSAGetOverlappedResult completed with the 16 bytes");
    expect(from.sin_family == AF_INET && ntohs(from.sin_port) == sport && fromlen == sizeof from,
           "the from address is the sender's 127.0.0.1 port");

    HMODULE ws = GetModuleHandleW(L"ws2_32.dll");
    typedef int(WSAAPI * SendToFn)(SOCKET, LPWSABUF, DWORD, LPDWORD, DWORD, const struct sockaddr *, int,
                                   LPWSAOVERLAPPED, LPWSAOVERLAPPED_COMPLETION_ROUTINE);
    typedef int(WSAAPI * SelectFn)(SOCKET, WSAEVENT, long);
    SendToFn wsasendto = (SendToFn)(void *)GetProcAddress(ws, "WSASendTo");
    SelectFn evsel = (SelectFn)(void *)GetProcAddress(ws, "WSAEventSelect");
    static const char msg2[] = "via-WSASendTo!!!";
    WSABUF sb = {16, (char *)msg2};
    DWORD sent = 0;
    expect(wsasendto(ss, &sb, 1, &sent, 0, (struct sockaddr *)&to, sizeof to, NULL, NULL) == 0 && sent == 16,
           "WSASendTo through GetProcAddress");
    char rb[64];
    struct sockaddr_in rf;
    int rfl = sizeof rf;
    int rn = -1;
    for (int i = 0; i < 100 && rn < 0; i++) {
        rn = recvfrom(rs, rb, sizeof rb, 0, (struct sockaddr *)&rf, &rfl);
        if (rn < 0)
            Sleep(10);
    }
    expect(rn == 16 && !memcmp(rb, msg2, 16) && ntohs(rf.sin_port) == sport, "recvfrom got the WSASendTo datagram");

    expect(connect(ss, (struct sockaddr *)&to, sizeof to) == 0, "connect the send socket");
    expect(send(ss, "connected-send!!", 16, 0) == 16, "send on the connected socket");
    rn = -1;
    for (int i = 0; i < 100 && rn < 0; i++) {
        rn = recv(rs, rb, sizeof rb, 0);
        if (rn < 0)
            Sleep(10);
    }
    expect(rn == 16 && !memcmp(rb, "connected-send!!", 16), "recv got the connected datagram");

    SOCKET es = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    WSAEVENT ev2 = WSACreateEvent();
    expect(evsel(es, ev2, FD_READ) == 0, "WSAEventSelect through GetProcAddress");

    SOCKET flood = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    struct sockaddr_in sink = loop_addr(sport);
    int okn = 0;
    for (int i = 0; i < 450; i++)
        okn += sendto(flood, "rate", 4, 0, (struct sockaddr *)&sink, sizeof sink) == 4;
    expect(okn == 450, "450 sendto calls on one socket");

    expect(closesocket(es) == 0 && closesocket(flood) == 0 && closesocket(dup) == 0, "closesocket");
    expect(closesocket(ss) == 0 && closesocket(rs) == 0, "closesocket the pump sockets");
    int cr = closesocket(rs);
    int ce = WSAGetLastError();
    expect(cr == SOCKET_ERROR && ce == WSAENOTSOCK, "a second closesocket fails with WSAENOTSOCK after the hook");
    WSACloseEvent(ev);
    WSACloseEvent(ev2);

    char ports[64];
    snprintf(ports, sizeof ports, "%u %u", rport, sport);
    HANDLE pf = CreateFileW(L"ports.txt", GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, 0, NULL);
    DWORD wn;
    WriteFile(pf, ports, (DWORD)strlen(ports), &wn, NULL);
    CloseHandle(pf);
    printf("%s (%d failure(s))\n", fails ? "FAILED" : "ALL PASSED", fails);
    ExitProcess(fails != 0);
}
