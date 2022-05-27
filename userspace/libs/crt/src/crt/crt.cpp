
using FuncPtr = void (*)();
using FuncWithArgPtr = void (*)(void *);

extern "C" {
extern const FuncPtr __preinit_array_start;
extern const FuncPtr __preinit_array_end;
extern const FuncPtr __init_array_start;
extern const FuncPtr __init_array_end;
extern const FuncPtr __fini_array_start;
extern const FuncPtr __fini_array_end;
}

namespace {
    class Spinlock final {
    public:
        void lock() noexcept {
          while (__atomic_exchange_n(&mLocked, true, __ATOMIC_ACQUIRE)) {}
        }

        void unlock() noexcept {
          mLocked = false;
          __atomic_store_n(&mLocked, false, __ATOMIC_RELEASE);
        }

    private:
        bool mLocked{false};
    };

    template <typename T>
    concept Lockable = requires(T t) {
      { t.lock() };
      { t.unlock() };
    };

    template<Lockable T>
    class UniqueLock final {
    public:
        explicit UniqueLock(T &lock) noexcept: mLock(lock) {
          mLock.lock();
        }

        ~UniqueLock() noexcept {
          mLock.unlock();
        }

    private:
        T &mLock;
    };
}

namespace crt {
    struct AtExitEntry final {
        FuncWithArgPtr fn;
        void *arg;
    };

    constexpr unsigned long long MAX_ATEXIT_HANDLERS = 50;
    AtExitEntry atExitHandlers[MAX_ATEXIT_HANDLERS];
    unsigned long long numAtExitHandlers = 0;

    void init() noexcept {
      const FuncPtr *ptr = &__preinit_array_start;
      while (ptr < &__preinit_array_end) {
        (*ptr++)();
      }

      ptr = &__init_array_start;
      while (ptr < &__init_array_end) {
        (*ptr++)();
      }
    }

    void fini() noexcept {
      for (long long i = numAtExitHandlers - 1; i >= 0; i--) {
        atExitHandlers[i].fn(atExitHandlers[i].arg);
      }

      const FuncPtr *ptr = &__fini_array_start;
      while (ptr < &__fini_array_end) {
        (*ptr++)();
      }
    }

    extern "C" void __cxa_atexit(FuncWithArgPtr fn, void *arg, void *dso_handle) {
      static Spinlock mutex;
      UniqueLock<Spinlock> l{mutex};

      atExitHandlers[numAtExitHandlers++] = AtExitEntry{
          .fn = fn,
          .arg = arg,
      };
    }
}  // namespace crt
