# /etc/profile.d/pac4cli -- set proxy
http_proxy=http://localhost:3128
https_proxy=http://localhost:3128
export http_proxy
export https_proxy

http_host=${http_proxy%:*}
http_host=${http_proxy##*/}

https_host=${https_proxy%:*}
https_host=${https_proxy##*/}

_JAVA_OPTIONS="${_JAVA_OPTIONS} -Dhttp.proxyHost=${http_host} -Dhttp.proxyPort=${http_proxy##*:} -Dhttps.proxyHost=${https_host} -Dhttps.proxyPort=${https_proxy##*:}"

export _JAVA_OPTIONS
