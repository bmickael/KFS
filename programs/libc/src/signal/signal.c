
#include "signal.h"

extern int user_sigaction(int signum, const struct sigaction *act, struct sigaction *oldact);
extern int user_signal(int signum, sighandler_t handler);
extern int errno;

/*
 * sigaction, rt_sigaction - examine and change a signal action
 */
int sigaction(int signum, const struct sigaction *act, struct sigaction *oldact)
{
	int ret = user_sigaction(signum, act, oldact);

	/*
	 * sigaction() returns 0 on success; on error, -1 is returned,
	 * and errno is set to indicate the error.
	 */
	if (ret < 0) {
		errno = -ret;
		return -1;
	} else {
		return 0;
	}
}

/*
 * signal - ANSI C signal handling
 */
sighandler_t signal(int signum, sighandler_t handler)
{
	int ret = user_signal(signum, handler);
	/*
	 * signal() returns the previous value of the signal handler, or SIG_ERR on error.
	 * In the event of an error, errno is set to indicate the cause.
	 */
	if (ret < 0) {
		errno = -ret;
		// TODO: Put SIG_ERR here
		return (sighandler_t)-1;
	} else {
		return handler;
	}
}
