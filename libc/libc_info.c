/*
 @author: david942j
 @improver: nosurfer
 Getting libc's [higher 2.19] information, such as:
 - libc version
 - main_arena offset
 - is tcache enabled?
 Sample Usage
 > ./libc_info
 > LD_LIBRARY_PATH=. ./libc_info
 > ./ld-linux.so.2 --library-path . ./libc_info
*/
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <gnu/libc-version.h>

#define SZ sizeof(size_t)
#define PAGE_SIZE 0x1000
#define LV gnu_get_libc_version()
#define PROTECT_PTR(pos, ptr) \
    ((__typeof (ptr)) ((((size_t) pos) >> 12) ^ ((size_t) ptr)))

unsigned get_minor() {
    static unsigned minor = 0;
    if (minor > 0) return minor;
    sscanf(LV, "%*u.%u", &minor);
    return minor;
}

void *search_head(size_t e) {
    e = (e >> 12) << 12; // page alignment
    while(strncmp((void*)e, "\177ELF", 4)) e -= PAGE_SIZE; // brute by page size
    return (void*) e;
}

void* main_arena_offset() {
    void **p = (void**)malloc(SZ * 128 * 2); // a large chunk
    void *z = malloc(SZ); // prevent p merge with top chunk
    *p = z; // prevent compiler optimize
    free(p); // now *p must be the pointer of the (chunk_ptr) unsorted bin
    z = get_minor() < 27u ? (void*)((*p) - (4 + 4 + SZ * 10)) : (void*)((*p) - (4 + 4 + SZ * 10 + 8));
    // mutex+flags+fastbin[] 2.20-2.26, 2.27+ need to -8
    void* a = search_head((size_t)__builtin_return_address(1));
    return (void*)(z - a);
}

int tcache_enable() {
    void **p = malloc(SZ * 32); // smallbin size
    *p = (void*) 0xdeadbeefu;
    // if tcache is enabled, this free will put p into tcache_entry;
    // otherwise, either merge with top_chunk or put into unsorted_bin
    free(p);
    if (get_minor() > 31u) { if (*p == PROTECT_PTR(p, NULL)); return 1; }
    // starting with libc 2.32 tcache pointers are encrypted with PROTECT_PTR
    else if (*p == 0) return 1; // tcache_entry, fd set as zero
    return 0;
}

int main(int argc, char **argv) {
    printf("{" \
        "\"libc_version\": %s," \
        "\"main_arena_offset\": %p," \
        "\"tcache_enable\": %s" \
        "}\n", LV, main_arena_offset(), tcache_enable() ? "true" : "false");
    return 0;
}