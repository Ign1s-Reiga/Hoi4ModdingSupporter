using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Hoi4ModdingSupporter.Controls {
    public sealed partial class HeaderText : UserControl {
        public static readonly DependencyProperty TitleTextProperty = DependencyProperty.Register(
            name: nameof(TitleText),
            propertyType: typeof(string),
            ownerType: typeof(HeaderText),
            typeMetadata: new PropertyMetadata(string.Empty)
        );

        public string TitleText {
            get => (string)GetValue(TitleTextProperty);
            set => SetValue(TitleTextProperty, value);
        }

        public static readonly DependencyProperty TextStyleProperty = DependencyProperty.Register(
            name: nameof(TextStyle),
            propertyType: typeof(Style),
            ownerType: typeof(HeaderText),
            typeMetadata: new PropertyMetadata(null)
        );

        public Style TextStyle {
            get => (Style)GetValue(TextStyleProperty);
            set => SetValue(TextStyleProperty, value);
        }

        public HeaderText() {
            InitializeComponent();
        }
    }
}
