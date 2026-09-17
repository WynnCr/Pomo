Name:           pomo
Version:        %{_version}
Release:        1%{?dist}
Summary:        A minimal and beautiful pomodoro timer

License:        MIT
URL:            https://github.com/WynnCr/Pomo

BuildArch:      x86_64

%description
A minimal and beautiful pomodoro timer built using GPUI


%prep

%build

%install
mkdir -p %{buildroot}/usr/bin

install -Dm755 \
    %{_topdir}/../rpm-root/usr/bin/pomo \
    %{buildroot}/usr/bin/pomo

%files
/usr/bin/pomo

%changelog
* Thu Sep 17 2026 WynnCr <amritanshukumar13012008@gmail.com> - 0.1.0-1
- Initial release
