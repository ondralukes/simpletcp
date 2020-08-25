#ifndef SIMPLETCP_UNIX_H
#define SIMPLETCP_UNIX_H

#ifdef _WIN32
typedef unsigned long long int Fd;
#else
typedef int Fd;
#endif

const short ev_pollin;
const short ev_pollout;
short c_poll(Fd fd, short events, int timeout);
#endif //SIMPLETCP_UNIX_H
