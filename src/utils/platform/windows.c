#include "interface.h"
#include <stdio.h>
#include <winsock2.h>

const short ev_pollin = POLLRDNORM;
const short ev_pollout = POLLWRNORM;

short c_poll(Fd fd, short events, int timeout){
    struct pollfd p;
    p.fd = fd;
    p.events = events;

    WSAPoll(&p, 1, timeout);
    return p.revents;
}