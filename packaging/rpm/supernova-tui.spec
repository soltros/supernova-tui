Name:           supernova-tui
Version:        0.1.1
Release:        1%{?dist}
Summary:        Ratatui TUI client for the Supernova music server
License:        GPL-3.0-or-later
URL:            https://github.com/soltros/supernova-tui
Requires:       mpv

%description
A full-featured terminal user interface (TUI) client for the Supernova music
server, built with Ratatui and supporting all Supernova plugins.

%install
rm -rf %{buildroot}
install -Dm755 %{_sourcedir}/supernova-tui %{buildroot}%{_bindir}/supernova-tui
install -Dm644 %{_sourcedir}/README.md %{buildroot}%{_docdir}/%{name}/README.md

%files
%{_bindir}/supernova-tui
%doc %{_docdir}/%{name}/README.md
