#include "interface.h"
#include <stdio.h>
#include <stdlib.h>
#include <winsock2.h>

const short ev_pollin = POLLRDNORM;
const short ev_pollout = POLLWRNORM;

int c_poll(Fd* fds, unsigned int length, short events, int timeout){
    struct pollfd* p = malloc(sizeof(struct pollfd) * length);
    for(unsigned int i = 0;i<length;i++){
        p[i].fd = fds[i];
        p[i].events = events;
    }

    WSAPoll(p, length, timeout);

    for(unsigned int i = 0;i<length;i++){
        if((p[i].revents & events) != 0){
            free(p);
            return i;
        }
    }
    free(p);
    return -1;
}