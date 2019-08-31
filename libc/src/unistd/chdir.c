#include <unistd.h>
#include <errno.h>

// The chdir() function shall cause the directory named by the
// pathname pointed to by the path argument to become the current
// working directory; that is, the starting point for path searches
// for pathnames not beginning with '/'.

#warning NOT IMPLEMENTED
#include <custom.h>

int chdir(const char *path)
{
	DUMMY
	(void)path;
	errno = ENOSYS;
	return -1;
}

