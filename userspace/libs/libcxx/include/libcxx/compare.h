#ifndef LIBCXX_COMPARE_H_
#define LIBCXX_COMPARE_H_

#include <libcxx/types.h>

namespace std {
    struct strong_ordering {
        inline constexpr static libcxx::i32 less = -1;
        inline constexpr static libcxx::i32 equal = 0;
        inline constexpr static libcxx::i32 equivalent = 0;
        inline constexpr static libcxx::i32 greater = 1;

        libcxx::i32 value;

        template<typename T>
        friend constexpr bool operator==(strong_ordering v, T t) noexcept;

        friend constexpr bool operator==(strong_ordering v, strong_ordering w) noexcept = default;

        template<typename T>
        friend constexpr bool operator<=(strong_ordering v, T t) noexcept;

        template<typename T>
        friend constexpr bool operator<=(T t, strong_ordering v) noexcept;

        template<typename T>
        friend constexpr bool operator>=(strong_ordering v, T t) noexcept;

        template<typename T>
        friend constexpr bool operator>=(T t, strong_ordering v) noexcept;

        template<typename T>
        friend constexpr bool operator<(strong_ordering v, T t) noexcept;

        template<typename T>
        friend constexpr bool operator<(T t, strong_ordering v) noexcept;

        template<typename T>
        friend constexpr bool operator>(strong_ordering v, T t) noexcept;

        template<typename T>
        friend constexpr bool operator>(T t, strong_ordering v) noexcept;
    };

    template<typename T>
    constexpr bool operator==(strong_ordering v, T t) noexcept {
      return v.value == 0;
    }

    template<typename T>
    constexpr bool operator<=(strong_ordering v, T t) noexcept {
      return (v.value == strong_ordering::less) || (v.value == strong_ordering::equal);
    }

    template<typename T>
    constexpr bool operator<=(T t, strong_ordering v) noexcept {
      return (v.value == strong_ordering::less) || (v.value == strong_ordering::equal);
    }

    template<typename T>
    constexpr bool operator>=(strong_ordering v, T t) noexcept {
      return (v.value == strong_ordering::greater) || (v.value == strong_ordering::equal);
    }

    template<typename T>
    constexpr bool operator>=(T t, strong_ordering v) noexcept {
      return (v.value == strong_ordering::greater) || (v.value == strong_ordering::equal);
    }

    template<typename T>
    constexpr bool operator<(strong_ordering v, T t) noexcept {
      return v.value == strong_ordering::less;
    }

    template<typename T>
    constexpr bool operator<(T t, strong_ordering v) noexcept {
      return v.value == strong_ordering::less;
    }

    template<typename T>
    constexpr bool operator>(strong_ordering v, T t) noexcept {
      return v.value == strong_ordering::greater;
    }

    template<typename T>
    constexpr bool operator>(T t, strong_ordering v) noexcept {
      return v.value == strong_ordering::greater;
    }
}

#endif  // LIBCXX_COMPARE_H_
