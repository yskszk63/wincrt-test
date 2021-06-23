#include <io.h>
#include <fcntl.h>
#include <errno.h>

int
main(int argc, char argv[]) {
    printf("%d %d %d", _O_BINARY, _O_TEXT, O_NOINHERIT);
    return 0;
}
