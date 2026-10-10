#!/usr/bin/env python3
import mythic_container
import asyncio

# Start Mythic container service and block forever
if __name__ == "__main__":
    mythic_container.mythic_service.start_and_run_forever()
