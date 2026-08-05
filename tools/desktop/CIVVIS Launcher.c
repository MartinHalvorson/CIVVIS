#include <errno.h>
#include <limits.h>
#include <mach-o/dyld.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

/*
 * Launch Services requires a native CFBundleExecutable on current macOS.
 * Keep policy in the adjacent Zsh launcher and make this wrapper do exactly
 * one thing: replace itself with that script.
 */
int main(void) {
    char executable[PATH_MAX];
    uint32_t executable_size = sizeof(executable);
    if (_NSGetExecutablePath(executable, &executable_size) != 0) {
        fputs("CIVVIS launcher path is too long\n", stderr);
        return 70;
    }

    char *separator = strrchr(executable, '/');
    if (separator == NULL) {
        fputs("CIVVIS launcher has no bundle directory\n", stderr);
        return 70;
    }
    *separator = '\0';

    char script[PATH_MAX];
    int written = snprintf(
        script,
        sizeof(script),
        "%s/%s",
        executable,
        "../Resources/CIVVIS Launcher.zsh"
    );
    if (written < 0 || (size_t)written >= sizeof(script)) {
        fputs("CIVVIS script path is too long\n", stderr);
        return 70;
    }

    execl("/bin/zsh", "zsh", script, (char *)NULL);
    fprintf(stderr, "CIVVIS could not start %s: %s\n", script, strerror(errno));
    return 71;
}
