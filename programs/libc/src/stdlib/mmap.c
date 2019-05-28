#include "stdlib.h"

extern void *user_mmap(void *addr, size_t length, int prot, int flags, int fd, off_t offset);

void *mmap(void *addr, size_t length, int prot, int flags, int fd, off_t offset)
{
	return user_mmap(addr, length, prot, flags, fd, offset);
}
