#ifndef LIBCXX_SPAN_H
#define LIBCXX_SPAN_H

#include <libcxx/iterator.h>
#include <libcxx/type_traits.h>
#include <libcxx/types.h>
#include <libcxx/array.h>

namespace libcxx {
    template<typename T, IteratorType U = Iterator<T>>
    class Span {
    public:
        constexpr Span() noexcept = default;

        constexpr Span(T *ptr, usize length) noexcept: mPtr(ptr), mLength(length) {}

        template<usize N>
        constexpr Span(Array<T, N> &array) noexcept: mPtr(array.data()), mLength(array.size()) {}

        template<SameAs<T> V = T>
        constexpr explicit Span(const Span<RemoveConstT<V>> &other) noexcept: mPtr(other.mPtr),
                                                                              mLength(other.mLength) {}

        constexpr Span(Span &&) = default;

        constexpr Span(const Span &) = default;

        constexpr Span &operator=(Span &&) = default;

        constexpr Span &operator=(const Span &) = default;

        constexpr Iterator<T> begin() const noexcept {
          return Iterator{&mPtr[0]};
        }

        constexpr Iterator<T> end() const noexcept {
          return Iterator{&mPtr[mLength]};
        }

        constexpr T &operator[](usize index) const noexcept {
          return mPtr[index];
        }

        constexpr usize size() const noexcept {
          return mLength;
        }

        constexpr Span from_offset(usize index) const noexcept {
          return Span{&mPtr[index], mLength - index};
        }

    private:
        T *mPtr{nullptr};
        usize mLength{0};
    };
}

#endif  // LIBCXX_SPAN_H
