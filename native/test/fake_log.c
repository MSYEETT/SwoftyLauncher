#define WIN32_LEAN_AND_MEAN
#include <windows.h>

/* Imports version.dll, so the loader attaches, and exits: the loader's log is all the fallback test reads. */
int main(void)
{
    GetFileVersionInfoSizeW(L"F1_25.exe", NULL);
    return 0;
}
