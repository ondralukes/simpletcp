#include "interface.h"
#include <stdio.h>
#include <stdlib.h>
#include <poll.h>

const short ev_pollin = POLLIN;
const short ev_pollout = POLLOUT;

int c_poll(Fd* fds, unsigned int length, short events, int timeout){
    struct pollfd* p = malloc(sizeof(struct pollfd) * length);
    for(unsigned int i = 0;i<length;i++){
        p[i].fd = fds[i];
        p[i].events = events;
    }

    poll(p, length, timeout);

    for(unsigned int i = 0;i<length;i++){
        if((p[i].revents & events) != 0){
            free(p);
            return i;
        }
    }
    free(p);
    return -1;
}