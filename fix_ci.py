import sys

with open('.github/workflows/ci.yml', 'r') as f:
    lines = f.readlines()

def move_lock_copy(start_search, end_search):
    global lines
    job_lines = lines[start_search:end_search]
    lock_line_idx = -1
    for i, line in enumerate(job_lines):
        if 'cp crates/Cargo.lock ci-build/' in line:
            lock_line_idx = i
            break

    if lock_line_idx != -1:
        lock_line = job_lines.pop(lock_line_idx)
        # Find where to insert: before first 'cargo add'
        insert_idx = -1
        for i, line in enumerate(job_lines):
            if 'cargo add' in line:
                # Need to find the start of the step (the - name: or just before the cargo add)
                # Actually, inserting just before the first cargo add is fine as long as we keep the step structure.
                # The lock copy is its own step.
                # Let's find the step before the first cargo add.
                for j in range(i, 0, -1):
                    if '- name:' in job_lines[j]:
                        insert_idx = j
                        break
                break

        if insert_idx != -1:
            job_lines.insert(insert_idx, lock_line)
            # Reconstruct lines
            lines = lines[:start_search] + job_lines + lines[end_search:]
            return True
    return False

# Job build-nodefault is roughly lines 297-340
# Job build-nostd is roughly lines 342-390
# We use grep to be sure
