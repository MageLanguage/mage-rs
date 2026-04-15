class Fibonacci {
  static get(n) {
    if (n < 2) return n
    return get(n - 1) + get(n - 2)
  }
}

var i = 0
while (i < 10) {
  Fibonacci.get(30)
i = i + 1
}
