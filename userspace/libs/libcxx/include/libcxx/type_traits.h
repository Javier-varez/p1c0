#ifndef LIBCXX_TYPE_TRAITS_H_
#define LIBCXX_TYPE_TRAITS_H_

namespace libcxx {
    template<typename T>
    struct RemoveConst final {
        using type = T;
    };

    template<typename T>
    struct RemoveConst<T const> final {
        using type = T;
    };

    template<typename T>
    using RemoveConstT = typename RemoveConst<T>::type;

    template<typename T, typename U>
    struct IsSame {
        inline constexpr static bool value = false;
    };

    template<typename T>
    struct IsSame<T, T> {
        inline constexpr static bool value = true;
    };

    template<typename T, typename U>
    inline constexpr static bool IsSameV = IsSame<T, U>::value;

    template<typename T, typename U>
    concept SameAs = IsSameV<T, U>;
}

#endif  // LIBCXX_TYPE_TRAITS_H_
