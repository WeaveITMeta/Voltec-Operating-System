package dev.v-os.qft;

/**
 * Configuration for QFT file operations.
 */
public class QftConfig {
    
    private int bondDimension;
    private boolean golayEnabled;
    private double truncationThreshold;
    
    /**
     * Creates a new configuration with specified values.
     * 
     * @param bondDimension Bond dimension for MPS compression (1-1024)
     * @param golayEnabled Enable Golay error correction
     * @param truncationThreshold Truncation threshold for SVD
     */
    public QftConfig(int bondDimension, boolean golayEnabled, double truncationThreshold) {
        this.bondDimension = bondDimension;
        this.golayEnabled = golayEnabled;
        this.truncationThreshold = truncationThreshold;
    }
    
    /**
     * Returns the default configuration.
     * 
     * @return Default QftConfig
     */
    public static QftConfig defaultConfig() {
        return new QftConfig(64, true, 1e-10);
    }
    
    public int getBondDimension() {
        return bondDimension;
    }
    
    public void setBondDimension(int bondDimension) {
        this.bondDimension = bondDimension;
    }
    
    public boolean isGolayEnabled() {
        return golayEnabled;
    }
    
    public void setGolayEnabled(boolean golayEnabled) {
        this.golayEnabled = golayEnabled;
    }
    
    public double getTruncationThreshold() {
        return truncationThreshold;
    }
    
    public void setTruncationThreshold(double truncationThreshold) {
        this.truncationThreshold = truncationThreshold;
    }
}
