#ifndef LIBCXX_ARRAY_H_
#define LIBCXX_ARRAY_H_

#include <libcxx/types.h>

namespace libcxx {
    template<typename T, usize N>
    class Array final {
    public:
        T __elem[N];

        T *data() {
          return &__elem[0];
        }

        const T *data() const {
          return &__elem[0];
        }

        usize size() const {
          return N;
        }

        T &operator[](usize index) {
          return __elem[index];
        }

        const T &operator[](usize index) const {
          return __elem[index];
        }

        Iterator <T> begin() {
          return Iterator<T>{&__elem[0]};
        }

        Iterator <T> end() {
          return Iterator<T>{&__elem[N]};
        }

        Iterator<const T> begin() const {
          return Iterator<const T>{&__elem[0]};
        }

        Iterator<const T> end() const {
          return Iterator<const T>{&__elem[N]};
        }

        Iterator<const T> cbegin() const {
          return Iterator<const T>{&__elem[0]};
        }

        Iterator<const T> cend() const {
          return Iterator<const T>{&__elem[N]};
        }
    };
}

#endif  // LIBCXX_ARRAY_H_
