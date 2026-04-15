def factorial(n):
    if n < 2:
        return 1
    return n * factorial(n - 1)


i = 0
while i < 10:
    factorial(20)
    i += 1
