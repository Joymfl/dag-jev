task_count = 0
with open("input.txt", "r", encoding="utf-8") as file:
    lines = file.readlines()
    task_count = len(lines)

with open("control.txt", "w", encoding="utf-8") as file:
    file.write("Control for tests. Template: dep_{i}_{j} = does i depend on j?")
    for i in range(task_count):
        for j in range(task_count):
            if i == j:
                continue
            file.write(f"\ndep_{i}_{j}:")
    print("Control written. Needs to be filled in by you.")
