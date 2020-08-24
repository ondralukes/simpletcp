#ifndef SIMPLETCP_UNIX_H
#define SIMPLETCP_UNIX_H

const short ev_pollin;
const short ev_pollout;
short c_poll(int fd, short events, int timeout);
#endif //SIMPLETCP_UNIX_H
