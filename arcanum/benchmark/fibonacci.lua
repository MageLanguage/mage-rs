local function fibonacci(n)
    if n < 2 then
        return n
    end
    return fibonacci(n - 1) + fibonacci(n - 2)
end

local i = 0
while i < 10 do
    fibonacci(30)
    i = i + 1
end
