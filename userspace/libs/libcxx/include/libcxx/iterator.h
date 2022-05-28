#ifndef LIBCXX_ITERATOR_H_
#define LIBCXX_ITERATOR_H_

#include <libcxx/type_traits.h>
#include <libcxx/types.h>
#include <libcxx/compare.h>

namespace  libcxx {
    template<typename T>
    concept IteratorType = requires(T t, T v) {
      { t++ } -> SameAs<T>;
      { t-- } -> SameAs<T>;
      { ++t } -> SameAs<T&>;
      { --t } -> SameAs<T&>;
      { t += 1 } -> SameAs<T&>;
      { t -= 1 } -> SameAs<T&>;
      { *t } -> SameAs<typename T::reference_type>;
      { t[2] } -> SameAs<typename T::reference_type>;
      { t == v } -> SameAs<bool>;
      { t != v } -> SameAs<bool>;
      { t > v } -> SameAs<bool>;
      { t >= v } -> SameAs<bool>;
      { t < v } -> SameAs<bool>;
      { t <= v } -> SameAs<bool>;
    };

    template <typename T>
    class Iterator {
    public:
        constexpr Iterator() noexcept = default;

        constexpr explicit Iterator(T *const ptr) noexcept: mPtr(ptr) {}

        constexpr Iterator(const Iterator&) noexcept = default;
        constexpr Iterator(Iterator&&) noexcept = default;

        constexpr Iterator& operator=(const Iterator&) noexcept = default;
        constexpr Iterator& operator=(Iterator&&) noexcept = default;

        constexpr explicit Iterator(Iterator<RemoveConst<T>> other) noexcept: mPtr(other.mPtr) {}

        constexpr T&operator*() const {
          return *mPtr;
        }

        constexpr T *operator->() const {
          return mPtr;
        }

        constexpr T &operator[](usize i) const {
          return mPtr[i];
        }

        constexpr Iterator operator++(int) {
          const auto copy = *this;
          mPtr++;
          return copy;
        }

        constexpr Iterator&operator++() {
          mPtr++;
          return *this;
        }

        constexpr Iterator operator--(int) {
          const auto copy = *this;
          mPtr--;
          return copy;
        }

        constexpr Iterator &operator--() {
          mPtr--;
          return *this;
        }

        constexpr Iterator &operator+=(usize step) {
          mPtr += step;
          return *this;
        }

        constexpr Iterator &operator-=(usize step) noexcept {
          mPtr -= step;
          return *this;
        }

        constexpr std::strong_ordering operator<=>(const Iterator&) const = default;

        using reference_type = T&;

    private:
        T *mPtr{nullptr};
    };

    // Just make sure Iterator is an IteratorType
    static_assert(IteratorType<Iterator<char>>);
}
#endif // LIBCXX_ITERATOR_H_
