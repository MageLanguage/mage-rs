def fibonacci(n):
    if n < 2:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)


i = 0
while i < 10:
    fibonacci(30)
    i += 1
