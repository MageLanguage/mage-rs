class Factorial {
  static get(n) {
    if (n < 2) return 1
    return n * get(n - 1)
  }
}

var i = 0
while (i < 10) {
  Factorial.get(20)
  i = i + 1
}
