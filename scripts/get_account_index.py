#!/usr/bin/env python3
"""
查询 Lighter 账户索引
需要: pip install lighter-sdk
"""
import asyncio
import sys
import lighter

BASE_URL = "https://mainnet.zklighter.elliot.ai"

async def get_account_index(l1_address: str):
    """通过 L1 地址查询账户索引"""
    client = lighter.ApiClient(lighter.Configuration(host=BASE_URL))
    try:
        resp = await lighter.AccountApi(client).accounts_by_l1_address(l1_address=l1_address)
        
        print(f"\nL1 地址: {l1_address}")
        print(f"主账户索引: {resp.master_account.index}")
        print(f"\n子账户列表:")
        for i, sub in enumerate(resp.sub_accounts):
            print(f"  [{i}] 索引: {sub.index}, 名称: {sub.name}")
        
        return resp.master_account.index
    except Exception as e:
        print(f"查询失败: {e}")
        return None
    finally:
        await client.close()

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("用法: python get_account_index.py <你的以太坊地址>")
        print("示例: python get_account_index.py 0x1234567890abcdef...")
        sys.exit(1)
    
    l1_address = sys.argv[1]
    asyncio.run(get_account_index(l1_address))
