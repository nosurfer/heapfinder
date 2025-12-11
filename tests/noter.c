#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <stdbool.h>

#define MAX_NOTES 10
#define MAX_NOTE_SIZE 0x500

const char* banner = "|=======[ Noter v1.33.7 ]=======| \n"
                     "[INFO] Booting Noter core...      \n"
                     "[INFO] Type 'help' for commands   \n";

const char* menu =   "Commands:                         \n"
                     "  new <size> - create new note    \n"
                     "  read <id>  - read a note        \n"
                     "  write <id> - write to a note    \n"
                     "  del <id>   - delete a note      \n"
                     "  help       - prints this        \n"
                     "  exit       - quits Noter 1.33.7 \n";

const char* prompt = "user@noter-console$ ";

typedef struct {
    char* data;
    size_t size;
} note;

note notes[MAX_NOTES];
unsigned note_counter;

int new_note(unsigned size) {
    char* ptr = NULL;

    if (note_counter >= MAX_NOTES) {
        puts("[ERROR] All notes are filled!!!");
        return -1;
    } 

    if (size <= 0 || size >= MAX_NOTE_SIZE) {
        puts("[ERROR] Invalid size!!!");
        return -1;
    }

    ptr = (char*)malloc(size);
    if (ptr == NULL) {
        perror("malloc");
        exit(EXIT_FAILURE);
    }

    puts("[INFO] Creating new note...");
    notes[note_counter].data = ptr;
    notes[note_counter].size = size;

    note_counter++;
    return 0;
}

void read_note(unsigned id) {
    if (id < 0 || id >= MAX_NOTES) {
        puts("[ERROR] Invalid note id!!!");
        return;
    }

    if (notes[id].data == NULL) {
        puts("[ERROR] Note doesn't exist!!!");
        return;
    }

    printf("%s", notes[id].data);
}

int write_note(unsigned id) {
    if (id < 0 || id >= MAX_NOTES) {
        puts("[ERROR] Invalid note id!!!");
        return -1;
    }

    if (notes[id].data == NULL) {
        puts("[ERROR] Note doesn't exist!!!");
        return -1;
    }

    printf("Input note data: ");
    if (read(0, notes[id].data, notes[id].size) == -1) {
        perror("read");
        exit(EXIT_FAILURE);
    }
    puts("[INFO] Writing down in a note...");

    return 0;
}

int del_note(unsigned id) {
    if (id < 0 || id >= MAX_NOTES) {
        puts("[ERROR] Invalid note id!!!");
        return -1;
    }
    if (notes[id].data == NULL) {
        puts("[ERROR] Note doesn't exist!!!");
        return -1;
    }
    puts("[INFO] Deleting a note...");
    free(notes[id].data);
    return 0;
}

int main(void) {
    unsigned param = 0;
    unsigned counter = 0;
    char choice[8] = { 0 };

    printf("%s", banner);
    while (true) {
        printf("%s", prompt);
        scanf("%7s[\n]", choice);

        if (strcmp(choice, "help") == 0) {
            printf("%s", menu);
            continue;
        } else if (strcmp(choice, "exit") == 0) 
            break;

        scanf("%u", &param);

        if (strcmp(choice, "new") == 0) {
            if (new_note(param) == 0) {
                puts("[INFO] Successfully created note!"); 
            }
        } else if (strcmp(choice, "read") == 0) {
            read_note(param);
        } else if (strcmp(choice, "write") == 0) {
            if (write_note(param) == 0) {
                puts("[INFO] Successfully wrote in a note!");
            }
        } else if (strcmp(choice, "del") == 0) {
            if (del_note(param) == 0) {
                puts("[INFO] Successfully deleted note!");
            }
        } else {
            printf("[ERROR] Invalid command option: %s\n", choice);
        }
    }
    
    puts("[INFO] Shutting down Noter...");
    exit(EXIT_SUCCESS);
}

__attribute__((constructor)) void
buf_init() {
    setvbuf(stdout, NULL, _IONBF, 0);
    setvbuf(stderr, NULL, _IONBF, 0);
    setvbuf(stdin, NULL, _IONBF, 0);
}
