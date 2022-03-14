typedef __UINT32_TYPE__ u32;
typedef __UINT64_TYPE__ u64;
typedef __SIZE_TYPE__ size_t;

#define CLINKAGE extern "C"

size_t strlen(const char *str) {
  if (str == nullptr) {
    return 0;
  }

  size_t size = 0;
  while (*str != '\0') {
    size++;
    str++;
  }

  return size;
}

void puts(const char *str) {
  const size_t string_length = strlen(str);
  asm volatile(
  "mov x0, %0\n"
  "mov x1, %1\n"
  "svc 6" : : "r" (str), "r" (string_length) : "x0", "x1" );
}

void sleep(const u64 time_us) {
  asm volatile(
  "mov x0, %0\n"
  "svc 2" : : "r" (time_us) : "x0");
}

CLINKAGE __attribute__((section(".init"))) int main() {
  while (true) {
    puts("Hi there!");
    sleep(1'000'000);
  }
  return 0;
}
