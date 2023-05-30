
#include <stdlib.h>

#ifdef __GLIBC__

#if !__GLIBC_PREREQ(2, 27)

#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

extern int memfd_create(const char *name, unsigned int flags);

int memfd_create(const char* name, unsigned int flags)
{
    return syscall(SYS_memfd_create, name, flags);
}

#endif

#endif
