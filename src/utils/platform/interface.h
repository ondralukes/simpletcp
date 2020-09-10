#ifndef SIMPLETCP_UNIX_H
#define SIMPLETCP_UNIX_H

#ifdef _WIN32
typedef unsigned long long int Fd;
#else
typedef int Fd;
#endif

const short ev_pollin;
const short ev_pollout;
int c_poll(Fd* fds, unsigned int length, short events, int timeout);
int c_poll_ev(Fd* fds, short *events, unsigned int length, int timeout);
#endif //SIMPLETCP_UNIX_H
