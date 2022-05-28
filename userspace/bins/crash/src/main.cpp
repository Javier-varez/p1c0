int main() {
    volatile int *ptr = nullptr;
    *ptr = 0xDEADC0DE;
    return 0;
}