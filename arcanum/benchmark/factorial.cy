fn factorial(n int) -> int:
    if n < 2:
        return 1
    return n * factorial(n - 1)

i := 0
while i < 10:
    result := factorial(20)
    i += 1
