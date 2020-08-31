#include "interface.h"
#include <stdio.h>
#include <poll.h>

const short ev_pollin = POLLIN;
const short ev_pollout = POLLOUT;

int c_poll(Fd* fds, unsigned int length, short events, int timeout){
    struct pollfd p[length];
    for(unsigned int i = 0;i<length;i++){
        p[i].fd = fds[i];
        p[i].events = events;
    }

    poll(p, length, timeout);

    for(unsigned int i = 0;i<length;i++){
        if(p[i].revents == events){
            return i;
        }
    }
    return -1;
}