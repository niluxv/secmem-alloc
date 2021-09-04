#include <stddef.h>

void zeroize_volatile(char *ptr, size_t count) {
    const char zero = 0;

    __asm__ volatile (
        "rep stosb %%al, (%%rdi)" :
        // input-output values (modified during execution)
        // `count` in the rcx register
        "+c" (count),
        // `ptr` int the rdi register
        "+D" (ptr) :
        // input (not modified)
        // zero byte to al (first byte of rax) register
        "a" (zero)
        :
        // we write to memory (pointed to by `ptr`)
        "memory"
    );
}
