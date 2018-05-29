import asyncio
import sys
import time
jokes = 0
bytes_ = 0

async def client(loop):
    global jokes, bytes_
    try:
        reader, _writer = await asyncio.open_connection("127.0.0.1", 12345, loop=loop)
        _writer.write(b"Hey\n\n")
        data = await reader.read()
    except Exception as e:
        print("IO Error: %s"%e,file=sys.stderr)
        return
    print("Received joke %d"%(jokes,))
    jokes+= 1
    bytes_+= len(data)

async def many_clients(loop, count):
    tasks =[loop.create_task(client(loop)) for _i in range(count)]
    await asyncio.gather(*tasks, loop=loop)

if __name__ == "__main__":
    try:
        count = int(sys.argv[1])
    except:
        count = 100

    loop = asyncio.get_event_loop()
    start = time.time()
    loop.run_until_complete(many_clients(loop,count))
    duration =  time.time() - start
    loop.close()
    print("received %d jokes, %d bytes in %f secs" % (jokes, bytes_, duration))


