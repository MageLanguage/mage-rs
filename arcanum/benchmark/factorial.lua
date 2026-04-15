local function factorial(n)
    if n < 2 then
        return 1
    end
    return n * factorial(n - 1)
end

local i = 0
while i < 10 do
    factorial(20)
    i = i + 1
end
