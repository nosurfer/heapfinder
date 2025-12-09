CC=gcc

GLIBC=$(shell pwd)
LD=$(GLIBC)/ld-linux-x86-64.so.2

CFLAGS=-g -Wl,-z,relro,-z,now -fstack-protector-all \
       -Wl,--rpath=$(GLIBC) \
       -Wl,--dynamic-linker=$(LD)

TARGET=noter.elf
SOURCES=noter.c

all:
	$(CC) $(CFLAGS) $(SOURCES) -o $(TARGET)

clean:
	rm -f $(TARGET)

.PHONY: all clean