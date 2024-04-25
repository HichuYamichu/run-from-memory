#include <stdint.h>

int foo(uint64_t x) {
    uint64_t y = 10;
    return x + 10;
}

int main() {
    uint64_t a = 1;
    uint64_t b = 2;
    uint64_t c = a + b;

    foo(c);

    return c;
}
