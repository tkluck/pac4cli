PYTHON ?= python3
PORT ?= 3128

env: requirements.txt
	virtualenv -p $(PYTHON) env
	env/bin/pip install -r requirements.txt
	PYTHON=`pwd`/env/bin/python make -C pacparser/src install-pymod

run:
	env/bin/python main.py -F DIRECT -p $(PORT)

check:
	./testrun.sh $(PORT)

install:
	systemctl stop pac4cli.service
	virtualenv -p $(PYTHON) /opt/pac4cli
	/opt/pac4cli/bin/pip install -r requirements.txt
	PYTHON=/opt/pac4cli/bin/python make -C pacparser/src install-pymod
	install -m 644 main.py proxy.py /opt/pac4cli
	install -m 644 pac4cli.service /lib/systemd/system
	install -m 755 trigger-pac4cli /etc/network/if-up.d
	install -m 755 pac4cli.sh /etc/profile.d
	systemctl enable pac4cli.service
	systemctl start pac4cli.service
