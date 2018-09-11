
#include "memory_manager.h"
#include "libft.h"

#define PHY_MAP_LOCATION	0x380000

static u8 *phy_map;

/*
 * Addressing of all 4go memory
 */
void		*get_physical_addr(u32 page_request)
{
	u32 phy_addr;

	if (page_request == 0)
		return (void *)MAP_FAILED;

	if (!IS_USABLE(phy_map, 1))
		return (void *)MAP_FAILED;

	phy_addr = get_mem_area(page_request, 1, 0, phy_map);

	return (void *)(phy_addr);
}

int		drop_physical_addr(void *addr)
{
	return free_mem_area((u32)addr, 1, 0, phy_map);
}

static size_t	count_bits(u32 ref)
{
	size_t count = 0;

	while (ref)
	{
		count++;
		ref >>= 1;
	}
	return count;
}

int		mark_physical_area(void *addr, u32 page_request)
{
	size_t	bitlen;
	u32	deep;

	if (page_request == 0)
		return -1;

	if (page_request <= GRANULARITY)
		deep = MAX_DEEP;
	else
	{
		page_request -= 1;
		bitlen = count_bits(page_request);
		deep = MAX_DEEP - bitlen + 1;
	}
	return mark_mem_area((u32)addr, 1, 0, deep, phy_map);
}

void		init_physical_map(void)
{
	phy_map = (u8 *)PHY_MAP_LOCATION;
	ft_memset(phy_map, 0, MAP_LENGTH);
}
