
const char s[] = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nam ac urna sit amet libero blandit efficitur tempus ac neque. Nullam at libero consequat, malesuada lorem id, dapibus urna. Integer vitae elit tincidunt, sagittis enim eu, dignissim lorem. Maecenas mollis nisi arcu, at lacinia odio sodales sit amet. Vivamus tristique magna vitae nunc congue, quis accumsan enim egestas. Suspendisse congue lorem elit, sed cursus nulla tempor ornare. Nam lobortis nisl nec justo lacinia viverra. Vivamus vel turpis diam. Quisque tincidunt ipsum congue mi gravida lobortis. Sed efficitur accumsan turpis quis mattis. Integer volutpat sed tortor at pretium. Aliquam consequat, nisl cursus consectetur sagittis, mi turpis eleifend nulla, et pharetra turpis leo ac tellus. Suspendisse eu magna vel enim auctor sagittis. Proin efficitur augue non molestie commodo. Donec metus sem, aliquam quis semper tincidunt, laoreet id mi. Cras porta gravida eros, at sagittis libero maximus eget. Sed tempus ligula tortor, sed porttitor magna volutpat condimentum. Vivamus sit amet nisl finibus nisl gravida rutrum. Ut a tincidunt sapien. Curabitur sed leo eget metus efficitur ultricies. Ut posuere sem quam, in venenatis dolor cursus ut. Nam id velit quis ipsum ultricies efficitur id in sapien. Nulla aliquet quam nulla, sit amet aliquet orci sodales in. Sed ut dui et augue sagittis imperdiet. Nunc nec metus sit amet magna cursus porttitor. Aliquam at nulla magna. Vivamus non malesuada nunc. Integer consectetur, neque id porta mollis, est magna lobortis elit, et sagittis massa arcu eget nulla. Aliquam sed blandit elit. Pellentesque quam nibh, lobortis ut euismod non, fermentum ac nibh. Nullam eu lorem nunc. Curabitur sodales viverra orci ac pharetra. Pellentesque imperdiet semper turpis, vel porttitor sem suscipit sed. Curabitur feugiat neque ut imperdiet tincidunt. Curabitur ac eros nec nulla dignissim mollis. Proin sit amet est dignissim, cursus lorem vitae, viverra leo. Fusce nec ultricies urna, nec vulputate neque. Nam eget sagittis metus. Vivamus maximus scelerisque eros, in tristique magna tincidunt ac. Nulla posuere, nisl pellentesque condimentum laoreet, nisi ante vulputate erat, non luctus metus sapien nec eros. Curabitur non fringilla justo. Nullam viverra consectetur diam at cursus. Vestibulum ut pharetra enim. In scelerisque ligula odio, vitae vehicula nisi mollis eu. Morbi mauris dolor, sagittis eget risus eu, elementum ornare velit. Aliquam metus lacus, ultricies at bibendum sit amet, ultricies quis lectus. Aenean libero risus, imperdiet sed ultricies eget, sagittis vitae justo. Vivamus pretium diam luctus sem fringilla, sed volutpat enim vestibulum. Nullam in pulvinar turpis. Morbi a nisl ex. Sed porta, lectus quis vehicula blandit, lorem nisi placerat orci, sed auctor urna odio vel ligula. Proin finibus neque in magna molestie, sit amet ullamcorper turpis auctor. Quisque venenatis cursus enim, non ullamcorper libero pretium in. Aenean vel massa felis. Cras nunc lacus, mattis eu maximus eu, tincidunt a massa. Donec nunc ex, facilisis eu maximus vitae, dictum ac arcu. Phasellus ut eleifend velit, at auctor sem. Ut at mi a mauris lacinia tincidunt sit amet a justo. Nullam congue nunc ut urna fermentum auctor. Sed pretium odio in lectus hendrerit, vel porta elit sollicitudin. Nunc elementum hendrerit ex. Ut sagittis nibh a sem pretium, at tempus nunc tempus. Mauris a maximus massa, sit amet scelerisque velit. Nullam eget erat consectetur, condimentum erat non, blandit purus. Donec efficitur, quam sit amet ultricies interdum, turpis massa gravida eros, vitae vehicula tortor diam id arcu. Fusce id tellus leo. Duis non tincidunt lectus, nec mattis dui. Nulla leo ante, commodo et nisl in, ultricies fermentum dui. Vivamus nisi sapien, volutpat a nunc quis, iaculis porta lectus. Pellentesque sed sapien massa. In at augue ultricies, suscipit ex quis, facilisis eros. Nam semper nec eros ut viverra. Donec eros risus, consectetur vitae velit vel, mollis gravida enim.";

#include <stdio.h>
#include <unistd.h>
#include <stdlib.h>
#include <errno.h>
#include <string.h>
#include <wait.h>

#include "tools.h"

int main(void)
{
	int fd[2];

	if (pipe(fd) == -1) {
		perror("pipe error");
		exit(1);
	}
	pid_t pid = fork();
	if (pid < 0) {
		perror("fork error");
		exit(1);
	} else if (pid == 0) {
		if (close(fd[0]) < 0) {
			perror("close failed");
			exit(1);
		}
		dup2(fd[1], 1);

		size_t total_len = strlen(s);
		size_t current = 0;
		char *ptr = (char *)s;

		srand16(0x42);

		while (current < total_len) {
			size_t trans = (size_t)rand16(32);
			if (trans > (total_len - current)) {
				trans = total_len - current;
			}
			int n = write(1, ptr, trans);
			if (n < 0) {
				perror("write");
				exit(1);
			}
			ptr += trans;
			current += trans;
		}
		sleep(2);
		dprintf(2, "write finished !\n");
		sleep(1);
		exit(0);
	} else {
		char buf[100];

		int n;
		char *ptr = (char *)s;

		if (close(fd[1]) < 0) {
			perror("close");
			exit(1);
		}
		while ((n = read(fd[0], buf, (size_t)rand16(31) + 1)) > 0) {
			buf[n] = '\0';
			printf("%s", buf);
			if (memcmp(buf, ptr, n) != 0) {
				printf("Bad Message received ! %s\n", buf);
				exit(1);
			}
			ptr += n;
		}
		printf("\n");
	}
	return 0;
}
