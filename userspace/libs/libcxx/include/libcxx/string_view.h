#ifndef LIBCXX_STRING_VIEW_H_
#define LIBCXX_STRING_VIEW_H_

#include <libcxx/span.h>
#include <libcxx/string.h>
#include <libcxx/types.h>

namespace libcxx {
    class StringView {
    public:
        constexpr StringView() = default;

        constexpr explicit StringView(const char *str) : mInner(str, strlen(str)) {}

        constexpr StringView(StringView &&) = default;

        constexpr StringView(const StringView &) = default;

        constexpr StringView &operator=(StringView &&) = default;

        constexpr StringView &operator=(const StringView &) = default;

        constexpr Iterator<const char> begin() const noexcept {
          return mInner.begin();
        }

        constexpr Iterator<const char> end() const noexcept {
          return mInner.end();
        }

        constexpr char operator[](usize index) const noexcept {
          return mInner[index];
        }

        constexpr usize size() const noexcept {
          return mInner.size();
        }

    private:
        Span<const char> mInner;
    };
}

#endif  // LIBCXX_STRING_VIEW_H_
