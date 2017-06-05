# /etc/profile.d/pac4cli -- set proxy
http_proxy=http://localhost:3128
https_proxy=http://localhost:3128
export http_proxy
export https_proxy

_JAVA_OPTIONS="${_JAVA_OPTIONS} -Dhttp.proxyHost=${http_proxy%:*} -Dhttp.proxyPort=${http_proxy##*:} -Dhttps.proxyHost=${https_proxy%:*} -Dhttps.proxyPort=${https_proxy##*:}"

export _JAVA_OPTIONS
