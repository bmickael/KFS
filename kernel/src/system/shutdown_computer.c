
#include "base_system.h"

void	shutdown_computer(void)
{
	struct registers reg;

	reg.eax = 0x530E;
	reg.ebx = 0x102;
	int8086(0x15, reg);

	reg.eax = 0x5300;
	reg.ebx = 0x0;
	int8086(0x15, reg);

	reg.eax = 0x5301;
	reg.ebx = 0x0;
	int8086(0x15, reg);

	reg.eax = 0x530E;
	reg.ebx = 0x102;
	int8086(0x15, reg);

	reg.eax = 0x5307;
	reg.ebx = 0x1;
	reg.ecx = 0x3;
	int8086(0x15, reg);
}
