using System;
using CommunityToolkit.Mvvm.ComponentModel;

namespace Hoi4ModdingSupporter.Models {
    public class LocalisationEntry : ObservableObject {
        private readonly Action? onChanged;
        private string key;
        private string version;
        private string value;

        public LocalisationEntry(string key, string version, string value, int lineIndex, Action? onChanged) {
            this.key = key;
            this.version = version;
            this.value = value;
            LineIndex = lineIndex;
            this.onChanged = onChanged;
        }

        public int LineIndex { get; }

        public string Key {
            get => key;
            set {
                if (SetProperty(ref key, value)) {
                    onChanged?.Invoke();
                }
            }
        }

        public string Version {
            get => version;
            set {
                if (SetProperty(ref version, value)) {
                    onChanged?.Invoke();
                }
            }
        }

        public string Value {
            get => value;
            set {
                if (SetProperty(ref this.value, value)) {
                    onChanged?.Invoke();
                }
            }
        }
    }
}
