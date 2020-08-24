#include "interface.h"
#include <stdio.h>
#include <poll.h>

const short ev_pollin = POLLIN;
const short ev_pollout = POLLOUT;

short c_poll(int fd, short events, int timeout){
    struct pollfd p;
    p.fd = fd;
    p.events = events;

    poll(&p, 1, timeout);
    return p.revents;
}